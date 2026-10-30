//! A extracao de um cliente, em fases:
//!
//! 1. cliente e membros: le o cadastro e, se for empresa ou departamento, as
//!    pessoas vinculadas a ela;
//! 2. listagem: ids dos tickets em `/tickets` e `/tickets/past`;
//! 3. tickets: GET por id de cada ticket que nao esta no cache da pasta;
//! 4. anexos (opcional): um download por anexo;
//! 5. arquivos: `tickets.json`, `tickets.csv` e as conversas em Markdown.
//!
//! Cancelar ou falhar no meio ainda gera os arquivos com o que ja foi salvo,
//! e a proxima extracao do mesmo cliente continua de onde parou.

use crate::movidesk::{Avisador, Aviso, Erro, Limitador, Movidesk, Pessoa, Rota, TicketListado};
use crate::saida::{self, Metadados, Situacao};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

/// Falhas seguidas ao baixar tickets antes de desistir: com a API fora do ar
/// cada ticket gastaria ~2 minutos em novas tentativas.
const FALHAS_SEGUIDAS_MAXIMAS: usize = 3;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opcoes {
    pub token: String,
    pub cliente_id: String,
    pub pasta_base: String,
    pub baixar_anexos: bool,
    /// Ignora o cache e baixa todos os tickets de novo.
    pub rebaixar_tudo: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Estado {
    Rodando,
    Concluida,
    Cancelada,
    Erro,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Fase {
    Cliente,
    Listagem,
    Tickets,
    Anexos,
    Arquivos,
    Fim,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progresso {
    pub estado: Estado,
    pub fase: Fase,
    pub mensagem: String,
    pub cliente: Option<Pessoa>,
    pub membros: usize,
    pub tickets_listados: usize,
    pub tickets_total: usize,
    pub tickets_processados: usize,
    pub tickets_baixados: usize,
    pub tickets_reaproveitados: usize,
    pub anexos_total: usize,
    pub anexos_processados: usize,
    pub anexos_baixados: usize,
    pub requisicoes: u32,
    pub requisicoes_por_minuto: u32,
    /// Segundos que faltam na espera atual (limite da API ou nova tentativa).
    pub espera_segundos: Option<u64>,
    pub restante_segundos: Option<u64>,
    pub iniciada_em: i64,
    pub atualizada_em: i64,
    pub pasta: Option<String>,
    pub caminho: Option<String>,
    pub erro: Option<String>,
    pub avisos: Vec<String>,
}

fn agora_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Progresso {
    fn novo(requisicoes_por_minuto: u32) -> Self {
        Self {
            estado: Estado::Rodando,
            fase: Fase::Cliente,
            mensagem: "Lendo o cadastro do cliente".into(),
            cliente: None,
            membros: 0,
            tickets_listados: 0,
            tickets_total: 0,
            tickets_processados: 0,
            tickets_baixados: 0,
            tickets_reaproveitados: 0,
            anexos_total: 0,
            anexos_processados: 0,
            anexos_baixados: 0,
            requisicoes: 0,
            requisicoes_por_minuto,
            espera_segundos: None,
            restante_segundos: None,
            iniciada_em: agora_ms(),
            atualizada_em: agora_ms(),
            pasta: None,
            caminho: None,
            erro: None,
            avisos: Vec::new(),
        }
    }
}

/// Recebe cada atualizacao do progresso (o lib.rs emite como evento Tauri).
pub type Emissor = Arc<dyn Fn(&Progresso) + Send + Sync>;

/// Progresso compartilhado entre a tarefa, o avisador do cliente HTTP e o
/// comando `status_extracao`.
#[derive(Clone)]
pub struct Painel {
    estado: Arc<Mutex<Progresso>>,
    emitir: Emissor,
    limitador: Arc<Limitador>,
    requisicoes_no_inicio: u32,
}

impl Painel {
    pub fn atual(&self) -> Progresso {
        self.estado.lock().expect("progresso").clone()
    }

    fn atualizar(&self, f: impl FnOnce(&mut Progresso)) {
        let copia = {
            let mut p = self.estado.lock().expect("progresso");
            f(&mut p);
            p.requisicoes = self.limitador.total() - self.requisicoes_no_inicio;
            p.atualizada_em = agora_ms();
            p.clone()
        };
        (self.emitir)(&copia);
    }

    fn avisar(&self, mensagem: String) {
        self.atualizar(|p| p.avisos.push(mensagem));
    }
}

pub struct Extracao {
    pub painel: Painel,
    pub cancel: CancellationToken,
}

impl Extracao {
    pub fn iniciar(opcoes: Opcoes, limitador: Arc<Limitador>, emitir: Emissor) -> Self {
        let painel = Painel {
            estado: Arc::new(Mutex::new(Progresso::novo(limitador.por_minuto()))),
            emitir,
            requisicoes_no_inicio: limitador.total(),
            limitador: limitador.clone(),
        };
        let cancel = CancellationToken::new();

        let painel_avisos = painel.clone();
        let avisador: Avisador = Arc::new(move |aviso| {
            match aviso {
            Aviso::Limite { segundos } => painel_avisos.atualizar(|p| p.espera_segundos = Some(segundos)),
            Aviso::Repetindo {
                segundos,
                tentativa,
                motivo,
            } => painel_avisos.atualizar(|p| {
                p.espera_segundos = Some(segundos);
                p.avisos.push(format!(
                    "Falha na requisição ({motivo}). Tentativa {tentativa}; nova tentativa em {segundos}s."
                ));
            }),
        }
        });
        let api = Movidesk::new(&opcoes.token, limitador, avisador, cancel.clone());

        let tarefa = Tarefa {
            api,
            painel: painel.clone(),
            opcoes,
            pasta: None,
            cliente: None,
            membros: Vec::new(),
            listados: None,
        };
        tauri::async_runtime::spawn(tarefa.rodar());
        Self { painel, cancel }
    }
}

struct Tarefa {
    api: Movidesk,
    painel: Painel,
    opcoes: Opcoes,
    pasta: Option<PathBuf>,
    cliente: Option<Pessoa>,
    membros: Vec<Pessoa>,
    listados: Option<Vec<TicketListado>>,
}

impl Tarefa {
    async fn rodar(mut self) {
        let resultado = self.executar().await;
        let (situacao, erro) = match &resultado {
            Ok(()) => (Situacao::Completa, None),
            Err(Erro::Cancelado) => (Situacao::Cancelada, None),
            Err(e) => (Situacao::Erro, Some(e.to_string())),
        };

        // Gera os arquivos mesmo cancelada ou com erro, com o que ja foi salvo.
        let mut erro_final = erro;
        if self.listados.is_some() {
            self.painel.atualizar(|p| {
                p.fase = Fase::Arquivos;
                p.mensagem = "Gerando tickets.json, tickets.csv e as conversas".into();
                p.espera_segundos = None;
            });
            if let Err(e) = self.consolidar(situacao, erro_final.clone()) {
                erro_final.get_or_insert(format!("Não foi possível gravar os arquivos: {e}"));
            }
        }

        let estado = match (situacao, &erro_final) {
            (Situacao::Cancelada, _) => Estado::Cancelada,
            (_, Some(_)) => Estado::Erro,
            _ => Estado::Concluida,
        };
        self.painel.atualizar(|p| {
            p.estado = estado;
            p.fase = Fase::Fim;
            p.espera_segundos = None;
            p.restante_segundos = None;
            p.erro = erro_final;
            p.mensagem = match estado {
                Estado::Concluida => "Extração concluída".into(),
                Estado::Cancelada => {
                    "Extração cancelada; os tickets já baixados foram salvos".into()
                }
                _ => "A extração parou com erro".into(),
            };
        });
    }

    async fn executar(&mut self) -> Result<(), Erro> {
        // 1. Cliente e membros
        let cliente = self
            .api
            .buscar_pessoa(&self.opcoes.cliente_id)
            .await?
            .ok_or_else(|| Erro::Api {
                status: 404,
                mensagem: format!("cliente {} não encontrado", self.opcoes.cliente_id),
            })?;
        let pasta = PathBuf::from(&self.opcoes.pasta_base).join(saida::nome_da_pasta(&cliente));
        std::fs::create_dir_all(&pasta).map_err(|e| {
            Erro::Resposta(format!(
                "não foi possível criar a pasta {}: {e}",
                pasta.display()
            ))
        })?;
        self.painel.atualizar(|p| {
            p.cliente = Some(cliente.clone());
            p.pasta = pasta.file_name().map(|n| n.to_string_lossy().into_owned());
            p.caminho = Some(pasta.to_string_lossy().into_owned());
        });
        self.pasta = Some(pasta.clone());
        self.cliente = Some(cliente.clone());

        let mut ids = vec![cliente.id.clone()];
        if cliente.e_organizacao() {
            self.painel.atualizar(|p| {
                p.mensagem = format!("Buscando as pessoas vinculadas a {}", cliente.nome)
            });
            self.membros = self.api.membros_da_organizacao(&cliente.id).await?;
            ids.extend(self.membros.iter().map(|m| m.id.clone()));
            let qtd = self.membros.len();
            self.painel.atualizar(|p| p.membros = qtd);
        }

        // 2. Listagem
        self.painel.atualizar(|p| {
            p.fase = Fase::Listagem;
            p.mensagem = "Listando os tickets atualizados nos últimos 90 dias".into();
        });
        let painel = self.painel.clone();
        let contar = move |n: usize| painel.atualizar(|p| p.tickets_listados += n);
        let mut todos: BTreeMap<i64, TicketListado> = BTreeMap::new();
        let mut juntar = |lista: Vec<TicketListado>| {
            for t in lista {
                todos
                    .entry(t.id)
                    .and_modify(|atual| {
                        // `/tickets` tem a versao mais nova; a data mais recente vence.
                        if t.rota == Rota::Atuais {
                            atual.rota = Rota::Atuais;
                        }
                        if t.ultima_atualizacao > atual.ultima_atualizacao {
                            atual.ultima_atualizacao = t.ultima_atualizacao.clone();
                        }
                    })
                    .or_insert(t);
            }
        };
        juntar(self.api.listar_tickets(Rota::Atuais, &ids, &contar).await?);
        if cliente.e_organizacao() {
            juntar(
                self.api
                    .listar_tickets_por_organizacao(&cliente.id, &contar)
                    .await?,
            );
        }
        self.painel
            .atualizar(|p| p.mensagem = "Listando os tickets mais antigos (arquivados)".into());
        juntar(
            self.api
                .listar_tickets(Rota::Passados, &ids, &contar)
                .await?,
        );

        // Mais novos primeiro: se cancelar no meio, o que ja saiu e o mais util.
        let mut listados: Vec<TicketListado> = todos.into_values().collect();
        listados.sort_by_key(|t| std::cmp::Reverse(t.id));
        let total = listados.len();
        self.listados = Some(listados.clone());
        self.painel.atualizar(|p| {
            p.tickets_total = total;
            p.fase = Fase::Tickets;
            p.mensagem = format!("{total} tickets encontrados; baixando as conversas");
        });

        // 3. Tickets
        let rpm = self.painel.limitador.por_minuto().max(1) as f64;
        let inicio_fase = Instant::now();
        let mut baixados_na_fase = 0usize;
        let mut falhas_seguidas = 0usize;
        let mut tickets_com_anexo = Vec::new();
        // Decide uma vez so quem precisa ir a API, para a estimativa de tempo
        // nao reler o cache inteiro a cada ticket.
        let precisa: Vec<bool> = listados
            .iter()
            .map(|t| self.opcoes.rebaixar_tudo || precisa_baixar(&pasta, t))
            .collect();
        let mut pendentes = precisa.iter().filter(|p| **p).count();
        for (i, item) in listados.iter().enumerate() {
            let salvo = saida::ler_ticket_salvo(&pasta, item.id);
            let ticket = if !precisa[i] {
                self.painel.atualizar(|p| p.tickets_reaproveitados += 1);
                salvo
            } else {
                self.painel.atualizar(|p| {
                    p.mensagem = format!("Baixando o ticket {} ({} de {total})", item.id, i + 1);
                });
                match self.baixar_ticket(item).await {
                    Ok(Some(t)) => {
                        saida::salvar_ticket(&pasta, item.id, &t).map_err(|e| {
                            Erro::Resposta(format!(
                                "não foi possível gravar o ticket {}: {e}",
                                item.id
                            ))
                        })?;
                        falhas_seguidas = 0;
                        baixados_na_fase += 1;
                        self.painel.atualizar(|p| p.tickets_baixados += 1);
                        Some(t)
                    }
                    Ok(None) => {
                        self.painel.avisar(format!(
                            "Ticket {} apareceu na listagem mas a API não o devolveu.",
                            item.id
                        ));
                        salvo
                    }
                    Err(Erro::Cancelado) => return Err(Erro::Cancelado),
                    Err(
                        e @ Erro::Api {
                            status: 401 | 403, ..
                        },
                    ) => return Err(e),
                    Err(e) => {
                        falhas_seguidas += 1;
                        if falhas_seguidas >= FALHAS_SEGUIDAS_MAXIMAS {
                            return Err(e);
                        }
                        self.painel
                            .avisar(format!("Ticket {} não foi baixado: {e}", item.id));
                        salvo
                    }
                }
            };
            if let Some(t) = &ticket {
                if !saida::anexos_do_ticket(t).is_empty() {
                    tickets_com_anexo.push(item.id);
                }
            }

            if precisa[i] {
                pendentes -= 1;
            }
            let por_requisicao = if baixados_na_fase >= 3 {
                inicio_fase.elapsed().as_secs_f64() / baixados_na_fase as f64
            } else {
                60.0 / rpm
            };
            let restante = (pendentes as f64 * por_requisicao.max(60.0 / rpm)) as u64;
            self.painel.atualizar(|p| {
                p.tickets_processados = i + 1;
                p.espera_segundos = None;
                p.restante_segundos = Some(restante);
            });
        }

        // 4. Anexos
        if self.opcoes.baixar_anexos {
            self.baixar_anexos(&pasta, &tickets_com_anexo, rpm).await?;
        }
        Ok(())
    }

    async fn baixar_ticket(&self, item: &TicketListado) -> Result<Option<Value>, Erro> {
        match self.api.buscar_ticket(item.rota, item.id).await? {
            Some(t) => Ok(Some(t)),
            // Pode ter ido para `/tickets/past` entre a listagem e o download.
            None if item.rota == Rota::Atuais => {
                self.api.buscar_ticket(Rota::Passados, item.id).await
            }
            None => Ok(None),
        }
    }

    async fn baixar_anexos(
        &self,
        pasta: &std::path::Path,
        tickets: &[i64],
        rpm: f64,
    ) -> Result<(), Erro> {
        let mut fila = Vec::new();
        for &id in tickets {
            if let Some(t) = saida::ler_ticket_salvo(pasta, id) {
                fila.extend(saida::anexos_do_ticket(&t).into_iter().map(|a| (id, a)));
            }
        }
        let total = fila.len();
        let pendentes_iniciais = fila
            .iter()
            .filter(|(id, a)| !saida::caminho_do_anexo(pasta, *id, &a.arquivo).exists())
            .count();
        self.painel.atualizar(|p| {
            p.fase = Fase::Anexos;
            p.anexos_total = total;
            p.mensagem = format!("Baixando {pendentes_iniciais} anexos ({total} no total)");
            p.restante_segundos = Some((pendentes_iniciais as f64 * 60.0 / rpm) as u64);
        });

        let mut pendentes = pendentes_iniciais;
        let mut falhas_seguidas = 0;
        for (i, (ticket_id, anexo)) in fila.iter().enumerate() {
            let destino = saida::caminho_do_anexo(pasta, *ticket_id, &anexo.arquivo);
            let nome = &anexo.nome;
            if !destino.exists() {
                self.painel.atualizar(|p| {
                    p.mensagem = format!("Baixando o anexo {nome} do ticket {ticket_id}");
                });
                match self.api.baixar_anexo(&anexo.hash).await {
                    Ok(bytes) => {
                        let gravado = destino
                            .parent()
                            .map(std::fs::create_dir_all)
                            .unwrap_or(Ok(()))
                            .and_then(|_| saida::escrever_atomico(&destino, &bytes));
                        if let Err(e) = gravado {
                            self.painel
                                .avisar(format!("Não foi possível gravar o anexo {nome}: {e}"));
                        } else {
                            self.painel.atualizar(|p| p.anexos_baixados += 1);
                        }
                        falhas_seguidas = 0;
                    }
                    Err(Erro::Cancelado) => return Err(Erro::Cancelado),
                    Err(
                        e @ Erro::Api {
                            status: 401 | 403, ..
                        },
                    ) => return Err(e),
                    Err(e) => {
                        falhas_seguidas += 1;
                        if falhas_seguidas >= FALHAS_SEGUIDAS_MAXIMAS {
                            return Err(e);
                        }
                        self.painel.avisar(format!(
                            "Anexo {nome} do ticket {ticket_id} não foi baixado: {e}"
                        ));
                    }
                }
                pendentes = pendentes.saturating_sub(1);
            }
            self.painel.atualizar(|p| {
                p.anexos_processados = i + 1;
                p.espera_segundos = None;
                p.restante_segundos = Some((pendentes as f64 * 60.0 / rpm) as u64);
            });
        }
        Ok(())
    }

    fn consolidar(&self, situacao: Situacao, erro: Option<String>) -> std::io::Result<()> {
        let (Some(pasta), Some(cliente), Some(listados)) =
            (&self.pasta, &self.cliente, &self.listados)
        else {
            return Ok(());
        };
        let mut tickets: Vec<Value> = listados
            .iter()
            .filter_map(|t| saida::ler_ticket_salvo(pasta, t.id))
            .collect();
        tickets.sort_by_key(saida::id_de);
        saida::gerar_consolidados(pasta, &tickets)?;

        let progresso = self.painel.atual();
        let meta = Metadados {
            versao: 1,
            cliente: cliente.clone(),
            membros: self.membros.clone(),
            situacao,
            iniciada_em: progresso.iniciada_em,
            concluida_em: agora_ms(),
            tickets_listados: listados.len(),
            tickets_salvos: tickets.len(),
            anexos_incluidos: self.opcoes.baixar_anexos,
            anexos_baixados: progresso.anexos_baixados,
            avisos: progresso.avisos.clone(),
            erro,
        };
        saida::salvar_metadados(pasta, &meta)
    }
}

fn precisa_baixar(pasta: &std::path::Path, item: &TicketListado) -> bool {
    match saida::ler_ticket_salvo(pasta, item.id) {
        Some(t) => {
            item.ultima_atualizacao.is_none()
                || t.get("lastUpdate").and_then(Value::as_str) != item.ultima_atualizacao.as_deref()
        }
        None => true,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Extracao completa contra a API real, numa pasta temporaria:
    /// `MOVIDESK_TESTE_CLIENTE=<id> [MOVIDESK_TESTE_ANEXOS=1] cargo test extrai -- --ignored --nocapture`.
    /// Rodar duas vezes confere o cache: a segunda so gasta a listagem.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn extrai_cliente_na_api_real() {
        let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../.env"));
        let token = std::env::var("MOVIDESK_API_KEY").expect("MOVIDESK_API_KEY");
        let cliente_id = std::env::var("MOVIDESK_TESTE_CLIENTE").expect("MOVIDESK_TESTE_CLIENTE");
        let base = std::env::temp_dir().join("movidesk-extrator-teste");
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let emitir: Emissor = Arc::new(move |p: &Progresso| {
            let _ = tx.send(p.clone());
        });
        let _extracao = Extracao::iniciar(
            Opcoes {
                token,
                cliente_id,
                pasta_base: base.to_string_lossy().into_owned(),
                baixar_anexos: std::env::var("MOVIDESK_TESTE_ANEXOS").is_ok_and(|v| v == "1"),
                rebaixar_tudo: false,
            },
            Arc::new(Limitador::new(
                crate::movidesk::REQUISICOES_POR_MINUTO_PADRAO,
            )),
            emitir,
        );
        let mut fim = None;
        while let Some(p) = rx.recv().await {
            eprintln!(
                "[{:?}] {} | tickets {}/{} baixados={} cache={} anexos {}/{} req={} espera={:?} eta={:?}",
                p.fase,
                p.mensagem,
                p.tickets_processados,
                p.tickets_total,
                p.tickets_baixados,
                p.tickets_reaproveitados,
                p.anexos_processados,
                p.anexos_total,
                p.requisicoes,
                p.espera_segundos,
                p.restante_segundos
            );
            if p.estado != Estado::Rodando {
                fim = Some(p);
                break;
            }
        }
        let fim = fim.expect("extracao terminou sem estado final");
        eprintln!("avisos: {:#?}\nerro: {:?}", fim.avisos, fim.erro);
        assert_eq!(fim.estado, Estado::Concluida);
        let pasta = PathBuf::from(fim.caminho.expect("caminho"));
        for arquivo in ["extracao.json", "tickets.json", "tickets.csv"] {
            assert!(pasta.join(arquivo).exists(), "{arquivo} nao foi gerado");
        }
        let conversas = std::fs::read_dir(pasta.join("conversas")).unwrap().count();
        assert_eq!(conversas, fim.tickets_total);
    }
}
