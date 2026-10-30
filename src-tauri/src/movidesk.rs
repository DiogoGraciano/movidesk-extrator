//! Cliente da API publica do Movidesk (https://api.movidesk.com/public/v1).
//!
//! Tres coisas da API moldam este modulo, todas conferidas contra a conta real:
//!
//! - O limite documentado e de 10 requisicoes por minuto, e 3 falhas seguidas
//!   bloqueiam o token por 60s, 120s e 300s (resposta 429 com `retry-after`).
//!   Toda chamada passa pelo [`Limitador`], que segura a requisicao ate caber
//!   na janela de um minuto, e as falhas esperam antes de tentar de novo.
//! - O servidor corta a consulta em ~30s e devolve 500. Filtrar tickets por
//!   `clients/any(c: c/organization/id eq ...)` em `/tickets/past` leva ~30s e
//!   estoura; filtrar por `c/id eq ...` leva 3 a 5s. Por isso a organizacao e
//!   consultada pelo proprio id mais o id de cada pessoa vinculada a ela.
//! - `/tickets` cobre os tickets atualizados nos ultimos 90 dias e
//!   `/tickets/past` o resto, com sobreposicao entre as duas. A listagem junta
//!   as duas rotas por id, e o ticket completo vem do GET por id, que e rapido
//!   (~1s) e traz todas as acoes da conversa.

use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

pub const URL_BASE: &str = "https://api.movidesk.com/public/v1";

/// Limite documentado pelo Movidesk. Contas com limite maior (negociado com o
/// suporte) podem subir o valor nas opcoes do app.
pub const REQUISICOES_POR_MINUTO_PADRAO: u32 = 10;

/// Maior pagina aceita pela API (`$top`).
const TAMANHO_PAGINA: usize = 1000;

/// Quantos ids de cliente vao num mesmo `clients/any(... or ...)`. Mantem a
/// URL curta e a consulta longe do timeout de ~30s do servidor.
const IDS_POR_CONSULTA: usize = 10;

const TENTATIVAS: u32 = 5;

/// Bloqueios (429) seguidos antes de desistir. O Movidesk aumenta o bloqueio a
/// cada rodada (60s, 120s, 300s); passar disso indica algo errado na conta.
const BLOQUEIOS_MAXIMOS: u32 = 4;

#[derive(Debug)]
pub enum Erro {
    Cancelado,
    /// Resposta 4xx que nao adianta repetir (token invalido, filtro errado).
    Api {
        status: u16,
        mensagem: String,
    },
    /// Falhou depois de todas as tentativas (rede, timeout, 5xx).
    Indisponivel(String),
    Resposta(String),
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erro::Cancelado => write!(f, "Extração cancelada."),
            Erro::Api { status, mensagem } if *status == 401 || *status == 403 => write!(
                f,
                "O Movidesk recusou a chave da API (HTTP {status}). Confira a chave em Configurações > Conta > Parâmetros > Ambiente. {mensagem}"
            ),
            Erro::Api { status, mensagem } => {
                write!(f, "O Movidesk respondeu HTTP {status}: {mensagem}")
            }
            Erro::Indisponivel(m) => write!(f, "A API do Movidesk não respondeu: {m}"),
            Erro::Resposta(m) => write!(f, "Resposta inesperada da API: {m}"),
        }
    }
}

pub type Resultado<T> = Result<T, Erro>;

/// Avisos que o cliente emite enquanto espera, para a tela explicar por que
/// a extracao parou de andar.
#[derive(Debug, Clone)]
pub enum Aviso {
    /// Esperando a janela do limite de requisicoes por minuto.
    Limite { segundos: u64 },
    /// Uma requisicao falhou e vai ser repetida depois de `segundos`.
    Repetindo {
        segundos: u64,
        tentativa: u32,
        motivo: String,
    },
}

pub type Avisador = Arc<dyn Fn(Aviso) + Send + Sync>;

/// Janela deslizante de um minuto: guarda o instante das ultimas requisicoes
/// e so libera a proxima quando houver vaga.
pub struct Limitador {
    por_minuto: AtomicU32,
    janela: Mutex<VecDeque<Instant>>,
    total: AtomicU32,
}

impl Limitador {
    pub fn new(por_minuto: u32) -> Self {
        Self {
            por_minuto: AtomicU32::new(por_minuto.max(1)),
            janela: Mutex::new(VecDeque::new()),
            total: AtomicU32::new(0),
        }
    }

    pub fn definir_por_minuto(&self, por_minuto: u32) {
        self.por_minuto.store(por_minuto.max(1), Ordering::Relaxed);
    }

    pub fn por_minuto(&self) -> u32 {
        self.por_minuto.load(Ordering::Relaxed)
    }

    /// Total de requisicoes liberadas desde que o app abriu.
    pub fn total(&self) -> u32 {
        self.total.load(Ordering::Relaxed)
    }

    async fn aguardar_vez(&self, cancel: &CancellationToken, avisar: &Avisador) -> Resultado<()> {
        loop {
            let espera = {
                let mut janela = self.janela.lock().await;
                let agora = Instant::now();
                let minuto = Duration::from_secs(60);
                while janela
                    .front()
                    .is_some_and(|t| agora.duration_since(*t) >= minuto)
                {
                    janela.pop_front();
                }
                if janela.len() < self.por_minuto() as usize {
                    janela.push_back(agora);
                    self.total.fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                }
                // Meio segundo de folga para o relogio do servidor.
                minuto - agora.duration_since(janela[0]) + Duration::from_millis(500)
            };
            avisar(Aviso::Limite {
                segundos: espera.as_secs().max(1),
            });
            dormir(espera, cancel).await?;
        }
    }

    /// Marca a janela como cheia depois de um 429, para nenhuma outra chamada
    /// furar o bloqueio enquanto ele dura.
    async fn registrar_bloqueio(&self) {
        let mut janela = self.janela.lock().await;
        let agora = Instant::now();
        janela.clear();
        for _ in 0..self.por_minuto() {
            janela.push_back(agora);
        }
    }
}

async fn dormir(duracao: Duration, cancel: &CancellationToken) -> Resultado<()> {
    tokio::select! {
        _ = cancel.cancelled() => Err(Erro::Cancelado),
        _ = tokio::time::sleep(duracao) => Ok(()),
    }
}

/// Rota de listagem de tickets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Rota {
    /// Atualizados nos ultimos 90 dias.
    Atuais,
    /// Mais antigos (arquivados).
    Passados,
}

impl Rota {
    fn caminho(self) -> &'static str {
        match self {
            Rota::Atuais => "tickets",
            Rota::Passados => "tickets/past",
        }
    }
}

/// Pessoa, empresa ou departamento do cadastro do Movidesk.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pessoa {
    pub id: String,
    pub nome: String,
    pub razao_social: Option<String>,
    pub cpf_cnpj: Option<String>,
    pub email: Option<String>,
    /// 1 = pessoa, 2 = empresa, 4 = departamento.
    pub tipo_pessoa: i64,
    /// 1 = agente, 2 = cliente, 3 = agente e cliente.
    pub tipo_perfil: i64,
    pub ativo: bool,
    /// Organizacoes as quais a pessoa esta vinculada (`relationships`).
    pub organizacoes: Vec<Vinculo>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vinculo {
    pub id: String,
    pub nome: String,
}

impl Pessoa {
    pub fn e_organizacao(&self) -> bool {
        self.tipo_pessoa == 2 || self.tipo_pessoa == 4
    }

    fn de_json(v: &Value) -> Option<Pessoa> {
        let texto = |chave: &str| {
            v.get(chave)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        };
        let id = texto("id")?;
        let email = v
            .get("emails")
            .and_then(Value::as_array)
            .and_then(|emails| {
                emails
                    .iter()
                    .find(|e| e.get("isDefault").and_then(Value::as_bool) == Some(true))
                    .or_else(|| emails.first())
            })
            .and_then(|e| e.get("email"))
            .and_then(Value::as_str)
            .map(String::from);
        let organizacoes = v
            .get("relationships")
            .and_then(Value::as_array)
            .map(|lista| {
                lista
                    .iter()
                    .filter_map(|r| {
                        Some(Vinculo {
                            id: r.get("id")?.as_str()?.to_string(),
                            nome: r
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Pessoa {
            nome: texto("businessName")
                .or_else(|| texto("corporateName"))
                .unwrap_or_else(|| id.clone()),
            id,
            razao_social: texto("corporateName"),
            cpf_cnpj: texto("cpfCnpj"),
            email,
            tipo_pessoa: v.get("personType").and_then(Value::as_i64).unwrap_or(1),
            tipo_perfil: v.get("profileType").and_then(Value::as_i64).unwrap_or(2),
            ativo: v.get("isActive").and_then(Value::as_bool).unwrap_or(true),
            organizacoes,
        })
    }
}

/// Ticket encontrado na listagem: o suficiente para decidir se precisa
/// baixar de novo e de qual rota.
#[derive(Debug, Clone)]
pub struct TicketListado {
    pub id: i64,
    pub ultima_atualizacao: Option<String>,
    pub rota: Rota,
}

#[derive(Clone)]
pub struct Movidesk {
    http: reqwest::Client,
    token: String,
    limitador: Arc<Limitador>,
    avisar: Avisador,
    cancel: CancellationToken,
    /// Busca e teste de chave: quem esta olhando a tela prefere um erro rapido
    /// a minutos de espera por novas tentativas ou pelo fim de um bloqueio.
    interativo: bool,
}

impl Movidesk {
    pub fn new(
        token: &str,
        limitador: Arc<Limitador>,
        avisar: Avisador,
        cancel: CancellationToken,
    ) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("movidesk-extrator/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(120))
            .build()
            .expect("cliente HTTP");
        Self {
            http,
            token: token.trim().to_string(),
            limitador,
            avisar,
            cancel,
            interativo: false,
        }
    }

    pub fn interativo(mut self) -> Self {
        self.interativo = true;
        self
    }

    async fn enviar(
        &self,
        caminho: &str,
        params: &[(&str, String)],
    ) -> Resultado<reqwest::Response> {
        let url = format!("{URL_BASE}/{caminho}");
        let mut tentativa = 0;
        let mut bloqueios = 0;
        let tentativas = if self.interativo { 2 } else { TENTATIVAS };
        loop {
            tentativa += 1;
            self.limitador
                .aguardar_vez(&self.cancel, &self.avisar)
                .await?;
            let envio = self
                .http
                .get(&url)
                .query(&[("token", self.token.as_str())])
                .query(params)
                .send();
            let resposta = tokio::select! {
                _ = self.cancel.cancelled() => return Err(Erro::Cancelado),
                r = envio => r,
            };

            let (motivo, espera) = match resposta {
                Ok(r) if r.status().is_success() => return Ok(r),
                Ok(r) if r.status().as_u16() == 429 => {
                    let segundos = r
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        .unwrap_or(60);
                    self.limitador.registrar_bloqueio().await;
                    bloqueios += 1;
                    if self.interativo || bloqueios > BLOQUEIOS_MAXIMOS {
                        return Err(Erro::Api {
                            status: 429,
                            mensagem: format!(
                                "o Movidesk bloqueou novas requisições por {segundos}s depois de falhas seguidas. Tente de novo em instantes."
                            ),
                        });
                    }
                    // O bloqueio nao conta como tentativa: so passa o tempo.
                    tentativa -= 1;
                    (
                        "a API pediu para esperar (HTTP 429)".to_string(),
                        segundos + 1,
                    )
                }
                Ok(r) if r.status().is_client_error() && r.status().as_u16() != 408 => {
                    let status = r.status().as_u16();
                    let corpo = r.text().await.unwrap_or_default();
                    return Err(Erro::Api {
                        status,
                        mensagem: mensagem_de_erro(&corpo),
                    });
                }
                Ok(r) => {
                    let status = r.status().as_u16();
                    let corpo = r.text().await.unwrap_or_default();
                    (
                        format!("HTTP {status} {}", mensagem_de_erro(&corpo)),
                        self.espera_apos_falha(tentativa),
                    )
                }
                Err(e) if e.is_timeout() => (
                    "a consulta demorou demais".to_string(),
                    self.espera_apos_falha(tentativa),
                ),
                Err(e) => (e.to_string(), self.espera_apos_falha(tentativa)),
            };

            if tentativa >= tentativas {
                return Err(Erro::Indisponivel(motivo));
            }
            (self.avisar)(Aviso::Repetindo {
                segundos: espera,
                tentativa,
                motivo,
            });
            dormir(Duration::from_secs(espera), &self.cancel).await?;
        }
    }

    /// Espera antes de repetir uma requisicao que falhou. Na extracao comeca
    /// alta de proposito: 3 falhas seguidas bloqueiam o token por um minuto.
    fn espera_apos_falha(&self, tentativa: u32) -> u64 {
        if self.interativo {
            return 3;
        }
        match tentativa {
            1 => 10,
            2 => 25,
            3 => 45,
            _ => 60,
        }
    }

    async fn get_json(&self, caminho: &str, params: &[(&str, String)]) -> Resultado<Value> {
        let resposta = self.enviar(caminho, params).await?;
        let corpo = resposta
            .text()
            .await
            .map_err(|e| Erro::Indisponivel(e.to_string()))?;
        serde_json::from_str(&corpo).map_err(|e| {
            let inicio: String = corpo.chars().take(200).collect();
            Erro::Resposta(format!("{e} ({inicio})"))
        })
    }

    /// Requisicao minima para conferir se a chave e aceita.
    pub async fn testar(&self) -> Resultado<()> {
        self.get_json("persons", &[("$select", "id".into()), ("$top", "1".into())])
            .await
            .map(|_| ())
    }

    /// Busca clientes pelo nome fantasia, razao social, e-mail, CPF/CNPJ ou id.
    pub async fn buscar_pessoas(&self, termo: &str) -> Resultado<Vec<Pessoa>> {
        let termo = termo.trim();
        if termo.is_empty() {
            return Ok(Vec::new());
        }
        let literal = literal_odata(termo);
        let mut condicoes = vec![
            format!("contains(businessName,{literal})"),
            format!("contains(corporateName,{literal})"),
        ];
        let digitos: String = termo.chars().filter(char::is_ascii_digit).collect();
        let so_numeros = termo
            .chars()
            .all(|c| c.is_ascii_digit() || ".-/ ".contains(c));
        if so_numeros && !digitos.is_empty() {
            condicoes.push(format!("id eq {}", literal_odata(&digitos)));
            if digitos.len() >= 11 {
                condicoes.push(format!("cpfCnpj eq {}", literal_odata(&digitos)));
            }
        }
        if termo.contains('@') {
            condicoes.push(format!("emails/any(e: e/email eq {literal})"));
        }
        let lista = self
            .get_json(
                "persons",
                &[
                    (
                        "$select",
                        "id,businessName,corporateName,cpfCnpj,personType,profileType,isActive"
                            .into(),
                    ),
                    ("$expand", "emails,relationships".into()),
                    ("$filter", condicoes.join(" or ")),
                    ("$top", "50".into()),
                ],
            )
            .await?;
        let mut pessoas: Vec<Pessoa> = como_lista(&lista)?
            .iter()
            .filter_map(Pessoa::de_json)
            .collect();
        // Empresas primeiro, depois por nome: quem busca um cliente quase
        // sempre quer a organizacao inteira.
        pessoas.sort_by(|a, b| {
            b.e_organizacao()
                .cmp(&a.e_organizacao())
                .then_with(|| a.nome.to_lowercase().cmp(&b.nome.to_lowercase()))
        });
        Ok(pessoas)
    }

    pub async fn buscar_pessoa(&self, id: &str) -> Resultado<Option<Pessoa>> {
        let v = self
            .get_json(
                "persons",
                &[
                    ("id", id.to_string()),
                    ("$expand", "emails,relationships".into()),
                ],
            )
            .await?;
        let v = match &v {
            Value::Array(lista) => lista.first().cloned().unwrap_or(Value::Null),
            outro => outro.clone(),
        };
        Ok(Pessoa::de_json(&v))
    }

    /// Pessoas e departamentos vinculados a organizacao (`relationships`).
    pub async fn membros_da_organizacao(&self, organizacao_id: &str) -> Resultado<Vec<Pessoa>> {
        let mut membros = Vec::new();
        let mut pular = 0;
        loop {
            let pagina = self
                .get_json(
                    "persons",
                    &[
                        (
                            "$select",
                            "id,businessName,corporateName,cpfCnpj,personType,profileType,isActive"
                                .into(),
                        ),
                        (
                            "$filter",
                            format!(
                                "relationships/any(r: r/id eq {})",
                                literal_odata(organizacao_id)
                            ),
                        ),
                        ("$top", TAMANHO_PAGINA.to_string()),
                        ("$skip", pular.to_string()),
                    ],
                )
                .await?;
            let lista = como_lista(&pagina)?;
            membros.extend(lista.iter().filter_map(Pessoa::de_json));
            if lista.len() < TAMANHO_PAGINA {
                break;
            }
            pular += TAMANHO_PAGINA;
        }
        Ok(membros)
    }

    /// Lista os tickets em que algum dos `clientes_ids` aparece como cliente.
    /// Pagina por `id gt` com `$orderby=id`, como a documentacao recomenda
    /// (o `$skip` fica lento em offsets grandes).
    pub async fn listar_tickets(
        &self,
        rota: Rota,
        clientes_ids: &[String],
        ao_listar: &(dyn Fn(usize) + Send + Sync),
    ) -> Resultado<Vec<TicketListado>> {
        let mut encontrados = BTreeMap::new();
        for grupo in clientes_ids.chunks(IDS_POR_CONSULTA) {
            let condicao = grupo
                .iter()
                .map(|id| format!("c/id eq {}", literal_odata(id)))
                .collect::<Vec<_>>()
                .join(" or ");
            self.listar_paginas(
                rota,
                &format!("clients/any(c: {condicao})"),
                &mut encontrados,
                ao_listar,
            )
            .await?;
        }
        Ok(encontrados.into_values().collect())
    }

    /// Tickets dos ultimos 90 dias em que o cliente aparece so pelo vinculo
    /// com a organizacao (por exemplo, alguem que ja saiu da empresa). Em
    /// `/tickets` essa consulta leva ~4s; em `/tickets/past` estoura o
    /// timeout do servidor, por isso so a rota atual.
    pub async fn listar_tickets_por_organizacao(
        &self,
        organizacao_id: &str,
        ao_listar: &(dyn Fn(usize) + Send + Sync),
    ) -> Resultado<Vec<TicketListado>> {
        let mut encontrados = BTreeMap::new();
        let filtro = format!(
            "clients/any(c: c/organization/id eq {})",
            literal_odata(organizacao_id)
        );
        self.listar_paginas(Rota::Atuais, &filtro, &mut encontrados, ao_listar)
            .await?;
        Ok(encontrados.into_values().collect())
    }

    async fn listar_paginas(
        &self,
        rota: Rota,
        filtro: &str,
        encontrados: &mut BTreeMap<i64, TicketListado>,
        ao_listar: &(dyn Fn(usize) + Send + Sync),
    ) -> Resultado<()> {
        let mut ultimo_id: i64 = 0;
        loop {
            let pagina = self
                .get_json(
                    rota.caminho(),
                    &[
                        ("$select", "id,lastUpdate".into()),
                        ("$filter", format!("({filtro}) and id gt {ultimo_id}")),
                        ("$orderby", "id".into()),
                        ("$top", TAMANHO_PAGINA.to_string()),
                    ],
                )
                .await?;
            let lista = como_lista(&pagina)?;
            for t in lista {
                let id = crate::saida::id_de(t);
                if id == 0 {
                    continue;
                }
                ultimo_id = ultimo_id.max(id);
                encontrados.entry(id).or_insert_with(|| TicketListado {
                    id,
                    ultima_atualizacao: t
                        .get("lastUpdate")
                        .and_then(Value::as_str)
                        .map(String::from),
                    rota,
                });
            }
            ao_listar(lista.len());
            if lista.len() < TAMANHO_PAGINA {
                return Ok(());
            }
        }
    }

    /// Ticket completo, com todas as acoes (conversa), anexos, apontamentos,
    /// historico de status e campos adicionais.
    pub async fn buscar_ticket(&self, rota: Rota, id: i64) -> Resultado<Option<Value>> {
        let v = self
            .get_json(rota.caminho(), &[("id", id.to_string())])
            .await?;
        Ok(match v {
            Value::Array(mut lista) if !lista.is_empty() => Some(lista.swap_remove(0)),
            Value::Object(_) => Some(v),
            _ => None,
        })
    }

    /// Baixa um anexo pelo hash (`actions[].attachments[].path`). A API
    /// redireciona para o storage do Movidesk e dele para o S3; o reqwest
    /// segue os dois redirecionamentos, e so a primeira chamada conta no
    /// limite da API.
    pub async fn baixar_anexo(&self, hash: &str) -> Resultado<Vec<u8>> {
        let resposta = self
            .enviar("storage/download", &[("id", hash.to_string())])
            .await?;
        let bytes = tokio::select! {
            _ = self.cancel.cancelled() => return Err(Erro::Cancelado),
            b = resposta.bytes() => b.map_err(|e| Erro::Indisponivel(e.to_string()))?,
        };
        Ok(bytes.to_vec())
    }
}

fn como_lista(v: &Value) -> Resultado<&Vec<Value>> {
    v.as_array()
        .ok_or_else(|| Erro::Resposta("esperava uma lista".into()))
}

/// Texto entre aspas simples para o `$filter` do OData (aspas dobradas).
fn literal_odata(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "''"))
}

fn mensagem_de_erro(corpo: &str) -> String {
    let corpo = corpo.trim();
    if let Ok(v) = serde_json::from_str::<Value>(corpo) {
        if let Some(m) = v
            .get("message")
            .or_else(|| v.get("Message"))
            .and_then(Value::as_str)
        {
            return m.to_string();
        }
    }
    corpo.chars().take(300).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn literal_odata_dobra_aspas() {
        assert_eq!(literal_odata("D'Ávila"), "'D''Ávila'");
    }

    /// Roda contra a API real com a chave do `.env`:
    /// `MOVIDESK_TESTE_BUSCA=<nome do cliente> cargo test busca_e_lista -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn busca_e_lista_na_api_real() {
        let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../.env"));
        let token = std::env::var("MOVIDESK_API_KEY").expect("MOVIDESK_API_KEY");
        let avisar: Avisador = Arc::new(|a| eprintln!("aviso: {a:?}"));
        let api = Movidesk::new(
            &token,
            Arc::new(Limitador::new(REQUISICOES_POR_MINUTO_PADRAO)),
            avisar,
            CancellationToken::new(),
        );
        let termo = std::env::var("MOVIDESK_TESTE_BUSCA").expect("MOVIDESK_TESTE_BUSCA");
        let pessoas = api.buscar_pessoas(&termo).await.unwrap();
        eprintln!("{pessoas:#?}");
        let alvo = pessoas.first().expect("nenhum cliente encontrado");
        let mut ids = vec![alvo.id.clone()];
        if alvo.e_organizacao() {
            ids.extend(
                api.membros_da_organizacao(&alvo.id)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|p| p.id),
            );
        }
        let nada = |_| {};
        let inicio = std::time::Instant::now();
        let atuais = api.listar_tickets(Rota::Atuais, &ids, &nada).await.unwrap();
        let passados = api
            .listar_tickets(Rota::Passados, &ids, &nada)
            .await
            .unwrap();
        eprintln!(
            "atuais={} passados={} em {:?}",
            atuais.len(),
            passados.len(),
            inicio.elapsed()
        );
        assert!(!atuais.is_empty() || !passados.is_empty());
    }
}
