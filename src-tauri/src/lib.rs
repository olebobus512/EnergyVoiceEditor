mod voice_catalog;
mod extensions;
mod pty;
mod audio;
mod commands;
mod evbank;
mod project_io;
mod settings;
mod shell;
mod voicebank_commands;
mod worker;

use commands::WorkerState;
use shell::ShellState;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(WorkerState::default())
        .manage(ShellState::default())
        .manage(pty::PtyState::default())
        .manage(voice_catalog::DownloadState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_voices,
            commands::get_voices_dir,
            commands::exit_app,
            voicebank_commands::evbank_list,
            voicebank_commands::evbank_install,
            voicebank_commands::evbank_verify,
            voicebank_commands::evbank_uninstall,
            voicebank_commands::evbank_export,
            voicebank_commands::evbank_pack,
            voicebank_commands::evbank_dir,
            voicebank_commands::evbank_preview,
            commands::synthesize,
            commands::read_audio,
            commands::preview_note,
            commands::export_audio,
            commands::get_settings,
            commands::save_settings,
            commands::import_audio,
            commands::render_mix,
            commands::get_app_info,
            project_io::save_project_file,
            project_io::open_project_file,
            voice_catalog::voice_catalog,
            voice_catalog::download_voice,
            voice_catalog::cancel_voice_download,
            voice_catalog::import_piper_voice,
            extensions::list_instruments,
            extensions::install_instrument,
            extensions::run_python_preset,
            pty::pty_start,
            pty::pty_write,
            pty::pty_resize,
            shell::shell_run,
            shell::shell_restart
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            pty::shutdown(&app_handle.state::<pty::PtyState>());
            commands::shutdown_worker(&app_handle.state::<WorkerState>());
            shell::shutdown_shell(&app_handle.state::<ShellState>());
        }
    });
}
