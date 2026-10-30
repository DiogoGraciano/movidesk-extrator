//! Movidesk Extrator — extrai todos os tickets de um cliente do Movidesk com o
//! historico completo de conversa e grava em JSON, CSV e Markdown.
//!
//! Todas as chamadas a API saem do Rust (e nao do webview) para nao depender
//! de CORS e para um unico [`Limitador`] controlar o limite de 10 requisicoes
//! por minuto da conta, valendo tanto para a busca quanto para a extracao.

mod extracao;
mod movidesk;
mod saida;

use extracao::{Emissor, Extracao, Opcoes, Progresso};
use movidesk::{Avisador, Limitador, Movidesk, Pessoa};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio_util::sync::CancellationToken;

const EVENTO_PROGRESSO: &str = "extracao://progresso";

struct Estado {
    limitador: Arc<Limitador>,
    extracao: Mutex<Option<Extracao>>,
}

fn cliente_avulso(token: &str, estado: &Estado) -> Result<Movidesk, String> {
    if token.trim().is_empty() {
        return Err("Informe a chave da API do Movidesk.".into());
    }
    let sem_avisos: Avisador = Arc::new(|_| {});
    Ok(Movidesk::new(
        token,
        estado.limitador.clone(),
        sem_avisos,
        CancellationToken::new(),
    )
    .interativo())
}

fn definir_limite(estado: &Estado, requisicoes_por_minuto: Option<u32>) {
    if let Some(rpm) = requisicoes_por_minuto {
        estado.limitador.definir_por_minuto(rpm.clamp(1, 600));
    }
}

/// Chave de `MOVIDESK_API_KEY`, para preencher o campo na primeira vez. Em
/// desenvolvimento o `.env` da raiz do projeto e carregado no `run()`.
#[tauri::command]
fn chave_padrao() -> Option<String> {
    std::env::var("MOVIDESK_API_KEY")
        .ok()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
}

#[tauri::command]
fn pasta_padrao() -> String {
    saida::pasta_base_padrao().to_string_lossy().into_owned()
}

/// Confere a chave com a requisicao mais barata possivel.
#[tauri::command]
async fn testar_chave(
    token: String,
    requisicoes_por_minuto: Option<u32>,
    estado: State<'_, Estado>,
) -> Result<(), String> {
    definir_limite(&estado, requisicoes_por_minuto);
    let api = cliente_avulso(&token, &estado)?;
    api.testar().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn buscar_clientes(
    token: String,
    termo: String,
    requisicoes_por_minuto: Option<u32>,
    estado: State<'_, Estado>,
) -> Result<Vec<Pessoa>, String> {
    definir_limite(&estado, requisicoes_por_minuto);
    let api = cliente_avulso(&token, &estado)?;
    api.buscar_pessoas(&termo).await.map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PedidoExtracao {
    #[serde(flatten)]
    opcoes: Opcoes,
    requisicoes_por_minuto: Option<u32>,
}

#[tauri::command]
fn iniciar_extracao(
    pedido: PedidoExtracao,
    app: AppHandle,
    estado: State<'_, Estado>,
) -> Result<Progresso, String> {
    let PedidoExtracao {
        opcoes,
        requisicoes_por_minuto,
    } = pedido;
    if opcoes.token.trim().is_empty() {
        return Err("Informe a chave da API do Movidesk.".into());
    }
    if opcoes.pasta_base.trim().is_empty() {
        return Err("Escolha a pasta onde salvar as extrações.".into());
    }
    let mut atual = estado.extracao.lock().map_err(|e| e.to_string())?;
    if atual
        .as_ref()
        .is_some_and(|e| e.painel.atual().estado == extracao::Estado::Rodando)
    {
        return Err("Já existe uma extração em andamento.".into());
    }
    definir_limite(&estado, requisicoes_por_minuto);
    let emitir: Emissor = Arc::new(move |p: &Progresso| {
        let _ = app.emit(EVENTO_PROGRESSO, p);
    });
    let nova = Extracao::iniciar(opcoes, estado.limitador.clone(), emitir);
    let progresso = nova.painel.atual();
    *atual = Some(nova);
    Ok(progresso)
}

#[tauri::command]
fn cancelar_extracao(estado: State<'_, Estado>) {
    if let Ok(atual) = estado.extracao.lock() {
        if let Some(e) = atual.as_ref() {
            e.cancel.cancel();
        }
    }
}

/// Ultimo progresso conhecido, para a tela se reidratar ao recarregar.
#[tauri::command]
fn status_extracao(estado: State<'_, Estado>) -> Option<Progresso> {
    estado
        .extracao
        .lock()
        .ok()
        .and_then(|atual| atual.as_ref().map(|e| e.painel.atual()))
}

#[tauri::command]
fn listar_extracoes(pasta_base: String) -> Result<Vec<saida::ExtracaoResumo>, String> {
    saida::listar_extracoes(&PathBuf::from(pasta_base)).map_err(|e| e.to_string())
}

#[tauri::command]
fn carregar_extracao(pasta_base: String, pasta: String) -> Result<saida::ExtracaoDetalhe, String> {
    saida::carregar_extracao(&PathBuf::from(pasta_base), &pasta).map_err(|e| e.to_string())
}

#[tauri::command]
fn carregar_ticket(
    pasta_base: String,
    pasta: String,
    id: i64,
) -> Result<saida::TicketCompleto, String> {
    saida::carregar_ticket(&PathBuf::from(pasta_base), &pasta, id).map_err(|e| e.to_string())
}

/// Abre com o aplicativo padrao um arquivo ou pasta de dentro da pasta das
/// extracoes. O caminho vem do webview, por isso a checagem em `dentro_da_base`.
#[tauri::command]
fn abrir_caminho(app: AppHandle, pasta_base: String, relativo: String) -> Result<(), String> {
    let base = PathBuf::from(&pasta_base);
    let alvo = saida::dentro_da_base(&base, &relativo).map_err(|e| e.to_string())?;
    if !alvo.exists() {
        return Err(format!("{} não existe.", alvo.display()));
    }
    app.opener()
        .open_path(alvo.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn mostrar_na_pasta(app: AppHandle, pasta_base: String, relativo: String) -> Result<(), String> {
    let base = PathBuf::from(&pasta_base);
    let alvo = saida::dentro_da_base(&base, &relativo).map_err(|e| e.to_string())?;
    app.opener()
        .reveal_item_in_dir(&alvo)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Em `tauri dev` le a chave do .env da raiz do projeto; no app instalado
    // so vale a variavel de ambiente, se existir.
    #[cfg(debug_assertions)]
    let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../.env"));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(Estado {
                limitador: Arc::new(Limitador::new(movidesk::REQUISICOES_POR_MINUTO_PADRAO)),
                extracao: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            chave_padrao,
            pasta_padrao,
            testar_chave,
            buscar_clientes,
            iniciar_extracao,
            cancelar_extracao,
            status_extracao,
            listar_extracoes,
            carregar_extracao,
            carregar_ticket,
            abrir_caminho,
            mostrar_na_pasta,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Movidesk Extrator");
}
