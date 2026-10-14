// No console window in release builds (a GUI program that opens a console flashes one).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    genslate_launcher_lib::run();
}
