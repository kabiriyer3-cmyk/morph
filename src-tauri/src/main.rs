#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
#![allow(dead_code)]

mod commands;
mod engine;
mod installer;
mod kinds;
mod mesh;
mod ops;
mod pdf;
mod tools;
mod util;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(engine::RunState::default())
        .invoke_handler(tauri::generate_handler![
            commands::system_info,
            commands::detect_tools,
            commands::scan_paths,
            commands::thumbnail,
            commands::probe,
            commands::mesh_preview,
            commands::read_text,
            commands::write_text,
            commands::list_archive,
            commands::open_path,
            commands::reveal_path,
            commands::clear_cache,
            commands::run_graph,
            commands::cancel_run,
            installer::tool_status,
            installer::install_tool,
            installer::remove_tool,
            installer::open_tools_folder,
            ops::sheet::sheet_preview,
            ops::pdftools::pdf_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Morph");
}
