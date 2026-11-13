//! The Tauri shell: commands and event forwarding only. All knowledge of Claude Code's files lives
//! in `virgil-core`; the pump thread drives it and forwards its events (data-contract 5).

mod commands;
mod pump;
mod tower;

use virgil_core::{AttentionItem, EventBatch, Hub, SourceStatus, EVENT_CHANNEL, STATUS_CHANNEL};
use pump::{Shared, Sink};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tower::Tower;

const MAIN_WINDOW: &str = "main";

struct WindowSink(AppHandle);

impl Sink for WindowSink {
    fn events(&self, batch: EventBatch) {
        if let Err(e) = self.0.emit_to(MAIN_WINDOW, EVENT_CHANNEL, batch) {
            eprintln!("virgil: emit {EVENT_CHANNEL}: {e}");
        }
    }
    fn status(&self, status: &SourceStatus) {
        if let Err(e) = self.0.emit_to(MAIN_WINDOW, STATUS_CHANNEL, status) {
            eprintln!("virgil: emit {STATUS_CHANNEL}: {e}");
        }
    }
    fn attention(&self, items: Vec<AttentionItem>) {
        self.0.state::<Tower>().update(&self.0, items);
    }
}

pub fn run() {
    let shared = Shared::new();
    let pump_shared = shared.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Tower::default())
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::get_status,
            commands::list_recordings,
            commands::open_recording,
            commands::replay_play,
            commands::replay_pause,
            commands::replay_seek,
            commands::replay_set_speed,
            commands::switch_to_live,
            commands::pick_recording_folder,
            commands::get_tool_history,
            commands::set_skip_gaps,
            commands::set_claude_dir,
            commands::pick_claude_dir,
            commands::ui_report,
            commands::get_attention,
            commands::set_attention_prefs,
            commands::mark_attention_seen,
        ])
        // Closing the window keeps the app in the menu bar; Quit is in the tray menu.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == MAIN_WINDOW {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            app.state::<Tower>().init(app.handle());
            // The app starts in live mode; the first scan runs on the pump thread.
            pump::spawn(pump_shared, WindowSink(app.handle().clone()), Hub::new_default);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Virgil");

    app.run(move |app, event| match event {
        RunEvent::Exit => shared.stop(),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => tower::show_main(app),
        _ => {
            let _ = app;
        }
    });
}
