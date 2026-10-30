//! Abre arquivos e pastas com o programa padrao do sistema.
//!
//! No AppImage, o AppRun aponta LD_LIBRARY_PATH, PATH, XDG_DATA_DIRS,
//! GSETTINGS_SCHEMA_DIR e outras variaveis para dentro do pacote, que traz as
//! bibliotecas do Ubuntu em que foi compilado. Os programas abertos pelo app
//! herdam esse ambiente: o Nautilus de uma distro mais nova carrega o
//! GStreamer empacotado e fecha na hora ("symbol lookup error"), sem que o app
//! fique sabendo. Por isso, no AppImage, o xdg-open roda sem essas entradas.

use std::path::Path;
use tauri::AppHandle;

#[cfg(any(target_os = "linux", test))]
use std::ffi::OsString;

/// Variaveis que o AppImage define sem apontar para dentro do pacote.
#[cfg(any(target_os = "linux", test))]
const SO_DO_APPIMAGE: [&str; 5] = [
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "GTK_THEME",
    "PYTHONDONTWRITEBYTECODE",
];

/// Abre `caminho` com o programa padrao, sem esperar ele fechar.
pub fn abrir(app: &AppHandle, caminho: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if let Some(appdir) = std::env::var_os("APPDIR") {
        return abrir_fora_do_appimage(caminho, Path::new(&appdir), std::env::vars_os());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(caminho.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// O ambiente de `vars` sem o que o AppImage acrescentou: as entradas dentro
/// de `appdir` saem das listas de caminhos (PATH, LD_LIBRARY_PATH...) e a
/// variavel some quando nao sobra nada nela.
#[cfg(any(target_os = "linux", test))]
fn ambiente_fora_do_appimage(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
    appdir: &Path,
) -> Vec<(OsString, OsString)> {
    let appdir_texto = appdir.to_string_lossy();
    vars.into_iter()
        .filter(|(nome, _)| !SO_DO_APPIMAGE.iter().any(|v| nome == v))
        .filter_map(|(nome, valor)| {
            let texto = valor.to_string_lossy();
            if !texto.contains(appdir_texto.as_ref()) {
                return Some((nome, valor));
            }
            let fora: Vec<&str> = texto
                .split(':')
                .filter(|p| !p.is_empty() && !Path::new(p).starts_with(appdir))
                .collect();
            (!fora.is_empty()).then(|| (nome, OsString::from(fora.join(":"))))
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn abrir_fora_do_appimage(
    caminho: &Path,
    appdir: &Path,
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let ambiente = ambiente_fora_do_appimage(vars, appdir);
    let mut erro = String::from("nenhum programa para abrir arquivos (xdg-open, gio...)");
    // xdg-open primeiro e depois as alternativas, na ordem do crate `open`.
    for mut comando in open::commands(caminho) {
        comando
            .env_clear()
            .envs(ambiente.iter().map(|(nome, valor)| (nome, valor)))
            // Fora do ponto de montagem do AppImage, que some quando o app fecha.
            .current_dir(dirs::home_dir().unwrap_or_else(|| "/".into()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let programa = comando.get_program().to_string_lossy().into_owned();
        let mut filho = match comando.spawn() {
            Ok(filho) => filho,
            Err(e) => {
                erro = format!("{programa}: {e}");
                continue;
            }
        };
        // O xdg-open pode ficar esperando o programa que ele abriu. Se ainda
        // estiver rodando depois de 250 ms (o mesmo prazo do crate `open`),
        // conta como aberto, e outra thread espera ele para nao virar zumbi.
        let limite = Instant::now() + Duration::from_millis(250);
        loop {
            match filho.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(status)) => {
                    erro = format!("{programa} terminou com {status}");
                    break;
                }
                Ok(None) if Instant::now() < limite => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Ok(None) => {
                    std::thread::spawn(move || filho.wait());
                    return Ok(());
                }
                Err(e) => {
                    erro = format!("{programa}: {e}");
                    break;
                }
            }
        }
    }
    Err(format!(
        "Não foi possível abrir {}: {erro}",
        caminho.display()
    ))
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn ambiente_sem_o_appimage() {
        let appdir = Path::new("/tmp/.mount_Movi123");
        let vars = [
            (
                "LD_LIBRARY_PATH",
                "/tmp/.mount_Movi123/usr/lib/:/tmp/.mount_Movi123//usr/lib64:/opt/outro/lib",
            ),
            (
                "PATH",
                "/tmp/.mount_Movi123/usr/bin/:/usr/local/bin:/usr/bin",
            ),
            (
                "XDG_DATA_DIRS",
                "/tmp/.mount_Movi123/usr/share:/usr/share:/usr/local/share",
            ),
            (
                "GSETTINGS_SCHEMA_DIR",
                "/tmp/.mount_Movi123//usr/share/glib-2.0/schemas",
            ),
            ("GTK_DATA_PREFIX", "/tmp/.mount_Movi123"),
            ("APPDIR", "/tmp/.mount_Movi123"),
            ("APPIMAGE", "/home/x/Movidesk.AppImage"),
            ("GTK_THEME", "Adwaita:dark"),
            ("HOME", "/home/x"),
            ("QT_QPA_PLATFORM", "wayland"),
        ]
        .map(|(nome, valor)| (OsString::from(nome), OsString::from(valor)));

        let limpo: HashMap<String, String> = ambiente_fora_do_appimage(vars, appdir)
            .into_iter()
            .map(|(nome, valor)| (nome.into_string().unwrap(), valor.into_string().unwrap()))
            .collect();

        assert_eq!(limpo["LD_LIBRARY_PATH"], "/opt/outro/lib");
        assert_eq!(limpo["PATH"], "/usr/local/bin:/usr/bin");
        assert_eq!(limpo["XDG_DATA_DIRS"], "/usr/share:/usr/local/share");
        assert_eq!(limpo["HOME"], "/home/x");
        assert_eq!(limpo["QT_QPA_PLATFORM"], "wayland");
        for nome in [
            "GSETTINGS_SCHEMA_DIR",
            "GTK_DATA_PREFIX",
            "APPDIR",
            "APPIMAGE",
            "GTK_THEME",
        ] {
            assert!(!limpo.contains_key(nome), "{nome} deveria sair do ambiente");
        }
    }
}
