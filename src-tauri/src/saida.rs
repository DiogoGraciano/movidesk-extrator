//! Arquivos gerados pela extracao. Cada cliente tem uma pasta propria dentro
//! da pasta base, e ela serve tambem de cache: na proxima extracao do mesmo
//! cliente, um ticket cuja `lastUpdate` nao mudou e lido do disco em vez de
//! gastar uma requisicao da API (o limite e de 10 por minuto).
//!
//! ```text
//! <base>/<Nome do cliente> (<id>)/
//!   extracao.json        metadados: cliente, membros, situacao, contagens
//!   tickets.json         todos os tickets completos, como a API devolve
//!   tickets.csv          um ticket por linha, separado por ";" (Excel pt-BR)
//!   tickets/<id>.json    cada ticket completo (o cache)
//!   conversas/<id> - <assunto>.md   a conversa legivel de cada ticket
//!   anexos/<ticket>/<acao>-<arquivo>  quando a opcao de anexos esta ligada
//! ```

use crate::movidesk::Pessoa;
use chrono::{Local, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const ARQUIVO_METADADOS: &str = "extracao.json";

pub fn pasta_base_padrao() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Movidesk Extrator")
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Situacao {
    Completa,
    Cancelada,
    Erro,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadados {
    pub versao: u32,
    pub cliente: Pessoa,
    pub membros: Vec<Pessoa>,
    pub situacao: Situacao,
    /// Epoch em milissegundos.
    pub iniciada_em: i64,
    pub concluida_em: i64,
    pub tickets_listados: usize,
    pub tickets_salvos: usize,
    pub anexos_incluidos: bool,
    pub anexos_baixados: usize,
    pub avisos: Vec<String>,
    pub erro: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtracaoResumo {
    /// Nome da pasta dentro da pasta base; e o identificador da extracao.
    pub pasta: String,
    pub caminho: String,
    pub metadados: Metadados,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtracaoDetalhe {
    pub pasta: String,
    pub caminho: String,
    pub metadados: Metadados,
    pub tickets: Vec<TicketResumo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketResumo {
    pub id: i64,
    pub protocolo: Option<String>,
    pub assunto: String,
    pub status: Option<String>,
    pub status_base: Option<String>,
    pub categoria: Option<String>,
    pub urgencia: Option<String>,
    pub servico: Option<String>,
    pub clientes: Vec<String>,
    pub responsavel: Option<String>,
    pub equipe: Option<String>,
    pub criado_em: Option<String>,
    pub ultima_atualizacao: Option<String>,
    pub resolvido_em: Option<String>,
    pub fechado_em: Option<String>,
    pub qtd_acoes: usize,
    pub qtd_anexos: usize,
    pub origem: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnexoLocal {
    pub acao_id: i64,
    pub hash: String,
    pub nome: String,
    /// Caminho relativo a pasta base, quando o arquivo foi baixado.
    pub caminho_relativo: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketCompleto {
    pub ticket: Value,
    pub anexos: Vec<AnexoLocal>,
}

/// Nome valido como arquivo no Linux, macOS e Windows, legivel (mantem acento).
pub fn nome_seguro(texto: &str, limite: usize) -> String {
    let limpo: String = texto
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let compacto = limpo.split_whitespace().collect::<Vec<_>>().join(" ");
    let cortado: String = compacto.chars().take(limite).collect();
    let final_ = cortado.trim().trim_end_matches(['.', ' ']).to_string();
    if final_.is_empty() {
        "sem nome".to_string()
    } else {
        final_
    }
}

pub fn nome_da_pasta(cliente: &Pessoa) -> String {
    format!(
        "{} ({})",
        nome_seguro(&cliente.nome, 80),
        nome_seguro(&cliente.id, 64)
    )
}

/// Resolve `relativo` dentro de `base`, recusando `..`, raiz e prefixos: os
/// comandos recebem esses caminhos do webview.
pub fn dentro_da_base(base: &Path, relativo: &str) -> io::Result<PathBuf> {
    let relativo = Path::new(relativo);
    let seguro = relativo
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    if !seguro || relativo.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "caminho fora da pasta das extrações",
        ));
    }
    Ok(base.join(relativo))
}

fn arquivo_do_ticket(pasta: &Path, id: i64) -> PathBuf {
    pasta.join("tickets").join(format!("{id}.json"))
}

pub fn ler_ticket_salvo(pasta: &Path, id: i64) -> Option<Value> {
    let texto = fs::read_to_string(arquivo_do_ticket(pasta, id)).ok()?;
    serde_json::from_str(&texto).ok()
}

pub fn salvar_ticket(pasta: &Path, id: i64, ticket: &Value) -> io::Result<()> {
    let destino = arquivo_do_ticket(pasta, id);
    fs::create_dir_all(destino.parent().expect("pasta tickets"))?;
    escrever_atomico(&destino, serde_json::to_string_pretty(ticket)?.as_bytes())
}

/// Grava num arquivo temporario e renomeia, para um cancelamento no meio nao
/// deixar um JSON pela metade no cache.
pub fn escrever_atomico(destino: &Path, conteudo: &[u8]) -> io::Result<()> {
    let temporario = destino.with_extension("parcial");
    fs::write(&temporario, conteudo)?;
    fs::rename(&temporario, destino)
}

pub fn caminho_do_anexo(pasta: &Path, ticket_id: i64, arquivo: &str) -> PathBuf {
    pasta
        .join("anexos")
        .join(ticket_id.to_string())
        .join(arquivo)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anexo {
    pub acao_id: i64,
    /// Hash usado no download (`attachments[].path`).
    pub hash: String,
    pub nome: String,
    /// Nome do arquivo em `anexos/<ticket>/`: `<acao>-<nome>`, numerado quando
    /// a mesma acao tem dois anexos com o mesmo nome (comum em chat, varios
    /// "image.png"). A numeracao segue a ordem da API, entao e estavel.
    pub arquivo: String,
}

/// Anexos de todas as acoes nao excluidas do ticket.
pub fn anexos_do_ticket(ticket: &Value) -> Vec<Anexo> {
    let mut lista = Vec::new();
    for acao in acoes(ticket)
        .iter()
        .filter(|a| !a.get("isDeleted").and_then(Value::as_bool).unwrap_or(false))
    {
        let acao_id = id_de(acao);
        let mut usados: Vec<String> = Vec::new();
        for anexo in acao
            .get("attachments")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(hash) = anexo.get("path").and_then(Value::as_str) else {
                continue;
            };
            let nome = anexo
                .get("fileName")
                .and_then(Value::as_str)
                .unwrap_or("anexo")
                .to_string();
            let seguro = nome_seguro(&nome, 120);
            let repeticoes = usados
                .iter()
                .filter(|u| u.eq_ignore_ascii_case(&seguro))
                .count();
            let arquivo = if repeticoes == 0 {
                format!("{acao_id}-{seguro}")
            } else {
                let (base, extensao) = match seguro.rsplit_once('.') {
                    Some((b, e)) if !b.is_empty() => (b.to_string(), format!(".{e}")),
                    _ => (seguro.clone(), String::new()),
                };
                format!("{acao_id}-{base} ({}){extensao}", repeticoes + 1)
            };
            usados.push(seguro);
            lista.push(Anexo {
                acao_id,
                hash: hash.to_string(),
                nome,
                arquivo,
            });
        }
    }
    lista
}

/// `id` do ticket ou da acao. A listagem devolve o id do ticket como numero,
/// mas o GET de um ticket devolve como texto ("469051").
pub fn id_de(v: &Value) -> i64 {
    match v.get("id") {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

fn acoes(ticket: &Value) -> &[Value] {
    ticket
        .get("actions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn salvar_metadados(pasta: &Path, meta: &Metadados) -> io::Result<()> {
    fs::create_dir_all(pasta)?;
    escrever_atomico(
        &pasta.join(ARQUIVO_METADADOS),
        serde_json::to_string_pretty(meta)?.as_bytes(),
    )
}

fn ler_metadados(pasta: &Path) -> io::Result<Metadados> {
    let texto = fs::read_to_string(pasta.join(ARQUIVO_METADADOS))?;
    serde_json::from_str(&texto).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Gera `tickets.json`, `tickets.csv` e as conversas em Markdown a partir
/// dos tickets ja salvos. `conversas/` e refeita do zero para nao sobrar o
/// arquivo de um ticket que mudou de assunto.
pub fn gerar_consolidados(pasta: &Path, tickets: &[Value]) -> io::Result<()> {
    escrever_atomico(
        &pasta.join("tickets.json"),
        serde_json::to_string_pretty(tickets)?.as_bytes(),
    )?;
    escrever_atomico(&pasta.join("tickets.csv"), gerar_csv(tickets).as_bytes())?;

    let conversas = pasta.join("conversas");
    if conversas.exists() {
        fs::remove_dir_all(&conversas)?;
    }
    fs::create_dir_all(&conversas)?;
    for ticket in tickets {
        let resumo = resumir(ticket);
        let nome = format!("{} - {}.md", resumo.id, nome_seguro(&resumo.assunto, 80));
        fs::write(conversas.join(nome), gerar_markdown(pasta, ticket))?;
    }
    Ok(())
}

pub fn listar_extracoes(base: &Path) -> io::Result<Vec<ExtracaoResumo>> {
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut lista = Vec::new();
    for entrada in fs::read_dir(base)? {
        let entrada = entrada?;
        if !entrada.file_type()?.is_dir() {
            continue;
        }
        let Ok(metadados) = ler_metadados(&entrada.path()) else {
            continue;
        };
        lista.push(ExtracaoResumo {
            pasta: entrada.file_name().to_string_lossy().into_owned(),
            caminho: entrada.path().to_string_lossy().into_owned(),
            metadados,
        });
    }
    lista.sort_by_key(|e| std::cmp::Reverse(e.metadados.concluida_em));
    Ok(lista)
}

pub fn carregar_extracao(base: &Path, pasta: &str) -> io::Result<ExtracaoDetalhe> {
    let dir = dentro_da_base(base, pasta)?;
    let metadados = ler_metadados(&dir)?;
    let tickets: Vec<Value> = match fs::read_to_string(dir.join("tickets.json")) {
        Ok(texto) => serde_json::from_str(&texto)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    let mut resumos: Vec<TicketResumo> = tickets.iter().map(resumir).collect();
    resumos.sort_by_key(|t| std::cmp::Reverse(t.id));
    Ok(ExtracaoDetalhe {
        pasta: pasta.to_string(),
        caminho: dir.to_string_lossy().into_owned(),
        metadados,
        tickets: resumos,
    })
}

pub fn carregar_ticket(base: &Path, pasta: &str, id: i64) -> io::Result<TicketCompleto> {
    let dir = dentro_da_base(base, pasta)?;
    let ticket = ler_ticket_salvo(&dir, id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("ticket {id} não está salvo"),
        )
    })?;
    let anexos = anexos_do_ticket(&ticket)
        .into_iter()
        .map(|a| {
            let arquivo = caminho_do_anexo(&dir, id, &a.arquivo);
            let caminho_relativo = arquivo
                .exists()
                .then(|| arquivo.strip_prefix(base).ok())
                .flatten()
                .map(|p| p.to_string_lossy().into_owned());
            AnexoLocal {
                acao_id: a.acao_id,
                hash: a.hash,
                nome: a.nome,
                caminho_relativo,
            }
        })
        .collect();
    Ok(TicketCompleto { ticket, anexos })
}

fn texto(v: &Value, chave: &str) -> Option<String> {
    v.get(chave)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn nome_de(v: Option<&Value>) -> Option<String> {
    v.and_then(|p| texto(p, "businessName").or_else(|| texto(p, "name")))
}

fn clientes(ticket: &Value) -> Vec<String> {
    ticket
        .get("clients")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let nome = nome_de(Some(c))?;
            Some(match nome_de(c.get("organization")) {
                Some(org) if org != nome => format!("{nome} ({org})"),
                _ => nome,
            })
        })
        .collect()
}

pub fn resumir(ticket: &Value) -> TicketResumo {
    TicketResumo {
        id: id_de(ticket),
        protocolo: texto(ticket, "protocol"),
        assunto: texto(ticket, "subject").unwrap_or_else(|| "(sem assunto)".into()),
        status: texto(ticket, "status"),
        status_base: texto(ticket, "baseStatus"),
        categoria: texto(ticket, "category"),
        urgencia: texto(ticket, "urgency"),
        servico: texto(ticket, "serviceFull")
            .or_else(|| {
                ticket
                    .get("serviceFull")
                    .and_then(Value::as_array)
                    .map(|l| {
                        l.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" > ")
                    })
            })
            .filter(|s| !s.is_empty()),
        clientes: clientes(ticket),
        responsavel: nome_de(ticket.get("owner")),
        equipe: texto(ticket, "ownerTeam"),
        criado_em: texto(ticket, "createdDate"),
        ultima_atualizacao: texto(ticket, "lastUpdate"),
        resolvido_em: texto(ticket, "resolvedIn"),
        fechado_em: texto(ticket, "closedIn"),
        qtd_acoes: acoes(ticket)
            .iter()
            .filter(|a| !a.get("isDeleted").and_then(Value::as_bool).unwrap_or(false))
            .count(),
        qtd_anexos: anexos_do_ticket(ticket).len(),
        origem: ticket.get("origin").and_then(Value::as_i64),
    }
}

/// A API devolve datas em UTC sem fuso ("2026-07-27T17:48:40.9947584").
pub fn data_local(utc: &str) -> String {
    let sem_fuso = utc.trim_end_matches('Z');
    NaiveDateTime::parse_from_str(sem_fuso, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(sem_fuso, "%Y-%m-%dT%H:%M:%S"))
        .map(|d| {
            Local
                .from_utc_datetime(&Utc.from_utc_datetime(&d).naive_utc())
                .format("%d/%m/%Y %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| utc.to_string())
}

pub fn nome_da_origem(origem: i64) -> &'static str {
    match origem {
        0 => "Primeira ação",
        1 => "Via web pelo cliente",
        2 => "Via web pelo agente",
        3 => "Recebido via e-mail",
        4 => "Gatilho do sistema",
        5 => "Chat (online)",
        6 => "Chat (offline)",
        7 => "E-mail enviado pelo sistema",
        8 => "Formulário de contato",
        9 => "Via API",
        10 => "Agendamento automático",
        11 => "Jira",
        12 => "Redmine",
        13 => "Telefone: recebida",
        14 => "Telefone: realizada",
        15 => "Telefone: perdida",
        16 => "Telefone: abandono na fila",
        17 => "Acesso remoto",
        18 => "WhatsApp",
        19 => "Integração Movidesk",
        20 => "Zenvia Chat",
        21 => "Telefone: não atendida",
        22 => "Facebook Messenger",
        23 => "WhatsApp Business Movidesk",
        24 => "Altu",
        25 => "WhatsApp Ativo",
        _ => "Outra origem",
    }
}

fn gerar_markdown(pasta: &Path, ticket: &Value) -> String {
    let r = resumir(ticket);
    let mut md = format!("# Ticket {} · {}\n\n", r.id, r.assunto.replace('\n', " "));
    let mut linha = |rotulo: &str, valor: Option<String>| {
        if let Some(v) = valor.filter(|v| !v.is_empty()) {
            md.push_str(&format!("- **{rotulo}:** {}\n", v.replace('\n', " ")));
        }
    };
    linha("Protocolo", r.protocolo.clone());
    linha(
        "Status",
        r.status.clone().map(|s| match &r.status_base {
            Some(b) if *b != s => format!("{s} ({b})"),
            _ => s,
        }),
    );
    linha("Cliente(s)", Some(r.clientes.join(", ")));
    linha(
        "Responsável",
        r.responsavel.clone().map(|resp| match &r.equipe {
            Some(eq) => format!("{resp} ({eq})"),
            None => resp,
        }),
    );
    linha("Categoria", r.categoria.clone());
    linha("Urgência", r.urgencia.clone());
    linha("Serviço", r.servico.clone());
    linha("Origem", r.origem.map(|o| nome_da_origem(o).to_string()));
    linha("Aberto em", r.criado_em.as_deref().map(data_local));
    linha("Resolvido em", r.resolvido_em.as_deref().map(data_local));
    linha("Fechado em", r.fechado_em.as_deref().map(data_local));
    linha(
        "Última atualização",
        r.ultima_atualizacao.as_deref().map(data_local),
    );
    let tags: Vec<&str> = ticket
        .get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !tags.is_empty() {
        md.push_str(&format!("- **Tags:** {}\n", tags.join(", ")));
    }

    let visiveis: Vec<&Value> = acoes(ticket)
        .iter()
        .filter(|a| !a.get("isDeleted").and_then(Value::as_bool).unwrap_or(false))
        .collect();
    md.push_str(&format!(
        "\n## Conversa ({} {})\n",
        visiveis.len(),
        if visiveis.len() == 1 {
            "ação"
        } else {
            "ações"
        }
    ));
    let todos_anexos = anexos_do_ticket(ticket);
    for acao in visiveis {
        let id = id_de(acao);
        let tipo = match acao.get("type").and_then(Value::as_i64) {
            Some(1) => "Interna",
            _ => "Pública",
        };
        let mut cabecalho = vec![format!("Ação {id}"), tipo.to_string()];
        if let Some(o) = acao.get("origin").and_then(Value::as_i64) {
            cabecalho.push(nome_da_origem(o).to_string());
        }
        if let Some(autor) = nome_de(acao.get("createdBy")) {
            cabecalho.push(autor);
        }
        if let Some(data) = texto(acao, "createdDate") {
            cabecalho.push(data_local(&data));
        }
        md.push_str(&format!("\n### {}\n\n", cabecalho.join(" · ")));
        if let Some(status) = texto(acao, "status") {
            let justificativa = texto(acao, "justification")
                .map(|j| format!(" ({j})"))
                .unwrap_or_default();
            md.push_str(&format!("_Status: {status}{justificativa}_\n\n"));
        }
        let descricao = texto(acao, "description").unwrap_or_default();
        md.push_str(&texto_markdown(&descricao));
        md.push('\n');

        // Link no formato <...> do CommonMark: aceita espaco e parenteses no nome.
        let anexos: Vec<String> = todos_anexos
            .iter()
            .filter(|a| a.acao_id == id)
            .map(|a| {
                if caminho_do_anexo(pasta, r.id, &a.arquivo).exists() {
                    format!("- [{}](<../anexos/{}/{}>)", a.nome, r.id, a.arquivo)
                } else {
                    format!("- {} (não baixado)", a.nome)
                }
            })
            .collect();
        if !anexos.is_empty() {
            md.push_str(&format!("\n**Anexos:**\n{}\n", anexos.join("\n")));
        }
    }
    md
}

/// Texto puro da acao como Markdown que preserva as linhas: cada quebra vira
/// quebra forcada ("  \n") e linhas que o Markdown leria como titulo, citacao
/// ou lista ganham escape. Transcricao de chat separa as mensagens com
/// "\n\r\n", que vira uma quebra so.
fn texto_markdown(descricao: &str) -> String {
    // CRLF vira um marcador antes, para o paragrafo de e-mail ("\r\n\r\n")
    // nao ser confundido com o separador do chat (LF seguido de CRLF).
    descricao
        .replace("\r\n", "\u{1}")
        .replace("\n\u{1}", "\n")
        .replace(['\u{1}', '\r'], "\n")
        .trim()
        .lines()
        .map(|linha| {
            let linha = linha.trim_end();
            let inicio = linha.trim_start();
            let recuo = &linha[..linha.len() - inicio.len()];
            let marcador = inicio.starts_with(['#', '>', '-', '*', '+', '=', '|'])
                || inicio
                    .split_once(['.', ')'])
                    .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
            if marcador {
                format!("{recuo}\\{inicio}")
            } else {
                linha.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("  \n")
}

fn gerar_csv(tickets: &[Value]) -> String {
    fn campo(v: &str) -> String {
        format!("\"{}\"", v.replace('"', "\"\"").replace(['\r', '\n'], " "))
    }
    let data = |d: &Option<String>| d.as_deref().map(data_local).unwrap_or_default();
    // BOM para o Excel abrir em UTF-8.
    let mut csv = String::from("\u{feff}");
    csv.push_str(
        "id;protocolo;assunto;status;status_base;categoria;urgencia;servico;clientes;responsavel;equipe;origem;aberto_em;resolvido_em;fechado_em;ultima_atualizacao;acoes;anexos\n",
    );
    for t in tickets {
        let r = resumir(t);
        let colunas = [
            r.id.to_string(),
            r.protocolo.clone().unwrap_or_default(),
            r.assunto.clone(),
            r.status.clone().unwrap_or_default(),
            r.status_base.clone().unwrap_or_default(),
            r.categoria.clone().unwrap_or_default(),
            r.urgencia.clone().unwrap_or_default(),
            r.servico.clone().unwrap_or_default(),
            r.clientes.join(", "),
            r.responsavel.clone().unwrap_or_default(),
            r.equipe.clone().unwrap_or_default(),
            r.origem
                .map(|o| nome_da_origem(o).to_string())
                .unwrap_or_default(),
            data(&r.criado_em),
            data(&r.resolvido_em),
            data(&r.fechado_em),
            data(&r.ultima_atualizacao),
            r.qtd_acoes.to_string(),
            r.qtd_anexos.to_string(),
        ];
        csv.push_str(
            &colunas
                .iter()
                .map(|c| campo(c))
                .collect::<Vec<_>>()
                .join(";"),
        );
        csv.push('\n');
    }
    csv
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_seguro_remove_caracteres_proibidos() {
        assert_eq!(
            nome_seguro("NF: 123/2024 \"urgente\"?", 80),
            "NF 123 2024 urgente"
        );
        assert_eq!(nome_seguro("   ", 80), "sem nome");
        assert_eq!(nome_seguro("abc...", 80), "abc");
    }

    #[test]
    fn dentro_da_base_recusa_subir_pastas() {
        let base = Path::new("/tmp/base");
        assert!(dentro_da_base(base, "../etc").is_err());
        assert!(dentro_da_base(base, "/etc").is_err());
        assert!(dentro_da_base(base, "").is_err());
        assert_eq!(
            dentro_da_base(base, "Cliente (1)/anexos/1/2-a.png").unwrap(),
            base.join("Cliente (1)/anexos/1/2-a.png")
        );
    }

    #[test]
    fn anexos_ignoram_acoes_excluidas() {
        let ticket = serde_json::json!({
            "id": 1,
            "actions": [
                {"id": 1, "isDeleted": false, "attachments": [{"fileName": "a.png", "path": "H1"}]},
                {"id": 2, "isDeleted": true, "attachments": [{"fileName": "b.png", "path": "H2"}]}
            ]
        });
        let anexos = anexos_do_ticket(&ticket);
        assert_eq!(anexos.len(), 1);
        assert_eq!(anexos[0].hash, "H1");
        assert_eq!(anexos[0].arquivo, "1-a.png");
    }

    #[test]
    fn anexos_com_mesmo_nome_na_acao_sao_numerados() {
        let ticket = serde_json::json!({
            "id": "1",
            "actions": [{"id": 3, "attachments": [
                {"fileName": "image.png", "path": "A"},
                {"fileName": "image.png", "path": "B"},
                {"fileName": "LEIAME", "path": "C"},
                {"fileName": "LEIAME", "path": "D"}
            ]}]
        });
        let arquivos: Vec<String> = anexos_do_ticket(&ticket)
            .into_iter()
            .map(|a| a.arquivo)
            .collect();
        assert_eq!(
            arquivos,
            ["3-image.png", "3-image (2).png", "3-LEIAME", "3-LEIAME (2)"]
        );
    }

    #[test]
    fn texto_markdown_preserva_linhas_e_escapa_marcadores() {
        assert_eq!(
            texto_markdown("Oi\n\r\n# 123 nota\r\n\r\n1. item\n- assinatura\nfim"),
            "Oi  \n\\# 123 nota  \n  \n\\1. item  \n\\- assinatura  \nfim"
        );
    }

    #[test]
    fn id_aceita_numero_ou_texto() {
        assert_eq!(id_de(&serde_json::json!({"id": 469051})), 469051);
        assert_eq!(id_de(&serde_json::json!({"id": "469051"})), 469051);
        assert_eq!(id_de(&serde_json::json!({})), 0);
    }

    #[test]
    fn data_local_aceita_fracao_de_sete_digitos() {
        let d = data_local("2026-07-27T17:48:40.9947584");
        assert!(
            d.starts_with("27/07/2026") || d.starts_with("28/07/2026"),
            "{d}"
        );
    }
}
