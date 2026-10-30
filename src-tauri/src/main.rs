// Impede a janela de console extra no Windows quando roda fora do terminal.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    movidesk_extrator_lib::run();
}
