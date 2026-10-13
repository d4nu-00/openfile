pub mod commands;
pub mod database_writes;
pub mod documents;
pub mod engine_commands;
pub mod registry;
pub mod save;
#[cfg(all(debug_assertions, target_os = "macos"))]
pub mod snapshot;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
#[derive(Default)]
pub struct ExitPolicy(AtomicBool);
/// What the command line asked to open: `chess-workbench [database] [--entry N]`.
/// `--node N` (debug builds only) also selects a node, for previews.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Startup {
    pub path: Option<String>,
    pub entry: Option<String>,
    pub node: Option<u32>,
    /// Debug builds on macOS: write a PNG of the window once this is shown.
    #[serde(skip)]
    pub snapshot: Option<std::path::PathBuf>,
}
impl Startup {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut startup = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--entry" => {
                    startup.entry = args
                        .next()
                        .filter(|v| v.parse::<u64>().is_ok_and(|n| n > 0))
                }
                "--node" if cfg!(debug_assertions) => {
                    startup.node = args.next().and_then(|v| v.parse().ok())
                }
                "--snapshot" if cfg!(debug_assertions) => {
                    startup.snapshot = args.next().map(Into::into)
                }
                // Platform launchers may add their own flags (e.g. -psn_ on macOS).
                flag if flag.starts_with('-') => {}
                path if startup.path.is_none() => startup.path = Some(path.into()),
                _ => {}
            }
        }
        startup
    }
}
/// Handed to the frontend once, so a reload does not reopen the file. The
/// snapshot destination stays here until the frontend reports it is ready.
#[derive(Default)]
pub struct StartupRequest(
    pub std::sync::Mutex<Option<Startup>>,
    pub std::sync::Mutex<Option<std::path::PathBuf>>,
);
pub fn run() {
    let startup = Startup::parse(std::env::args().skip(1));
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
        .manage(StartupRequest(
            std::sync::Mutex::new(Some(startup.clone())),
            std::sync::Mutex::new(startup.snapshot),
        ))
        .manage(std::sync::Arc::new(
            engine_commands::EngineManager::default(),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::open_database,
            commands::create_database,
            commands::save_game,
            commands::replace_game,
            commands::set_deleted,
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
            commands::startup_request,
            commands::startup_shown,
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
