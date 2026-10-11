pub mod commands;
pub mod documents;
pub mod engine_commands;
pub mod registry;
pub mod save;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
#[derive(Default)]
pub struct ExitPolicy(AtomicBool);
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(|app| {
            let menu = tauri::menu::Menu::default(app)?;
            #[cfg(target_os = "macos")]
            {
                // The predefined Cocoa Quit item calls terminate: directly,
                // bypassing ExitRequested. Route Command-Q through our guard.
                if let Some(tauri::menu::MenuItemKind::Submenu(sub)) = menu.items()?.first() {
                    let last = sub.items()?.len().saturating_sub(1);
                    sub.remove_at(last)?;
                    sub.insert(
                        &tauri::menu::MenuItem::with_id(
                            app,
                            "guarded-quit",
                            "Quit Chess Workbench",
                            true,
                            Some("CmdOrCtrl+Q"),
                        )?,
                        last,
                    )?;
                }
            }
            Ok(menu)
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "guarded-quit" {
                app.exit(0);
            }
        })
        .manage(std::sync::Arc::new(registry::Registry::default()))
        .manage(ExitPolicy::default())
        .manage(std::sync::Arc::new(
            engine_commands::EngineManager::default(),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::open_database,
            commands::read_page,
            commands::load_entry,
            commands::position_at,
            commands::close_database,
            commands::close_document,
            commands::open_pgn,
            commands::load_pgn_game,
            commands::close_pgn,
            commands::edit_document,
            commands::save_document,
            commands::finish_close,
            engine_commands::choose_engine,
            engine_commands::set_engine_option,
            engine_commands::start_analysis,
            engine_commands::stop_analysis,
            engine_commands::close_engine
        ])
        .plugin(
            tauri::plugin::Builder::<tauri::Wry, ()>::new("local-navigation")
                .on_navigation(|_, url| {
                    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
                        || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"))
                        || (cfg!(debug_assertions)
                            && url.scheme() == "http"
                            && url.host_str() == Some("127.0.0.1")
                            && url.port() == Some(1420))
                })
                .build(),
        )
        .build(tauri::generate_context!())
        .expect("Cannot build workbench")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let _ = app
                    .state::<std::sync::Arc<engine_commands::EngineManager>>()
                    .shutdown();
            }

            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && !app.state::<ExitPolicy>().0.load(Ordering::Acquire)
                && app
                    .state::<std::sync::Arc<registry::Registry>>()
                    .has_dirty_documents()
            {
                api.prevent_exit();
                let _ = app.emit("workbench-exit-request", ());
            }
        });
}
