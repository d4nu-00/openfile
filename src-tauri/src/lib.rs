pub mod analysis_jobs;
pub mod blunder_check;
pub mod centipawn;
pub mod centipawn_list;
pub mod clipboard;
pub mod cloud;
pub mod commands;
pub mod convert;
pub mod database_writes;
pub mod deep_analysis;
pub mod documents;
pub mod dossier;
pub mod engine_commands;
pub mod error_report;
pub mod evaluation_profile;
pub mod export;
pub mod library;
pub mod lists;
pub mod maintenance;
pub mod names;
pub mod openings;
pub mod playing_style;
pub mod prepare;
pub mod recovery;
pub mod registry;
pub mod repertoire;
pub mod repertoire_workspace;
pub mod replay_training;
pub mod save;
pub mod searches;
pub mod settings;
#[cfg(all(debug_assertions, target_os = "macos"))]
pub mod snapshot;
pub mod statistics;
pub mod style_dimensions;
pub mod tablebases;
pub mod trees;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
#[derive(Default)]
pub struct ExitPolicy(AtomicBool);
/// What the command line asked to open: `openfile [database] [--entry N]`.
/// `--node N` (debug builds only) also selects a node, for previews.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Startup {
    pub path: Option<String>,
    pub entry: Option<String>,
    pub node: Option<u32>,
    /// Debug builds: a command id to run once the entry is shown.
    pub command: Option<String>,
    /// Debug builds: a list column to sort by first, e.g. `white` or
    /// `date:desc`.
    pub sort: Option<String>,
    /// Debug builds: a new game from this position, e.g. for snapshots of
    /// tablebase or Chess960 positions.
    pub fen: Option<String>,
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
                "--command" if cfg!(debug_assertions) => startup.command = args.next(),
                "--sort" if cfg!(debug_assertions) => startup.sort = args.next(),
                "--fen" if cfg!(debug_assertions) => startup.fen = args.next(),
                // Platform launchers may add their own flags (e.g. -psn_ on macOS).
                flag if flag.starts_with('-') => {}
                path if startup.path.is_none() => startup.path = Some(path.into()),
                _ => {}
            }
        }
        startup
    }
}
/// The bundle identifier before the app was renamed OpenFile.
const OLD_IDENTIFIER: &str = "org.chessworkbench.desktop";

/// Moves the app's folder from its old name, when only the old one exists,
/// so caches and settings survive the rename. Failure leaves both alone.
pub fn move_old_folder(old: &std::path::Path, new: &std::path::Path) {
    if old.is_dir() && !new.exists() {
        let _ = std::fs::rename(old, new);
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
                            "Quit OpenFile",
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
        .manage(std::sync::Arc::new(recovery::RecoveryJobs::default()))
        .manage(std::sync::Arc::new(cloud::CloudDownloads::default()))
        .setup(|app| {
            // The app was first called Chess Workbench: move its folders once.
            for base in [app.path().data_dir(), app.path().config_dir()]
                .into_iter()
                .flatten()
            {
                move_old_folder(
                    &base.join(OLD_IDENTIFIER),
                    &base.join(&app.config().identifier),
                );
            }
            // Search boosters and opening trees (in a sibling folder) are the
            // app's own files, never beside a source.
            if let Ok(dir) = app.path().app_data_dir() {
                app.state::<std::sync::Arc<registry::Registry>>()
                    .set_index_dir(dir.join("search-boosters"));
                app.state::<repertoire_workspace::RepertoireWorkspace>()
                    .set_dir(dir.join("repertoire-workspace"));
                app.state::<replay_training::ReplayTrainingStore>()
                    .set_dir(dir.join("replay-training-sessions"));
            }
            if let Ok(dir) = app.path().app_config_dir() {
                app.state::<settings::Settings>().set_dir(&dir);
            }
            // Tablebases open in the background; a missing folder is shown
            // as no tables, not as an error.
            let folder = app.state::<settings::Settings>().load().tablebase_dir;
            let tables = app
                .state::<std::sync::Arc<tablebases::Tablebases>>()
                .inner()
                .clone();
            std::thread::spawn(move || {
                let _ = tables.set_dir(folder.as_deref());
            });
            Ok(())
        })
        .manage(settings::Settings::default())
        .manage(std::sync::Arc::new(blunder_check::BlunderJobs::default()))
        .manage(std::sync::Arc::new(error_report::ErrorReportJobs::default()))
        .manage(std::sync::Arc::new(repertoire::RepertoireJobs::default()))
        .manage(repertoire_workspace::RepertoireWorkspace::default())
        .manage(replay_training::ReplayTrainingStore::default())
        .manage(std::sync::Arc::new(centipawn::CentipawnJobs::default()))
        .manage(std::sync::Arc::new(
            evaluation_profile::InstantJobs::default(),
        ))
        .manage(std::sync::Arc::new(
            centipawn_list::CentipawnListJob::default(),
        ))
        .manage(std::sync::Arc::new(deep_analysis::DeepJobs::default()))
        .manage(std::sync::Arc::new(tablebases::Tablebases::default()))
        .manage(ExitPolicy::default())
        .manage(StartupRequest(
            std::sync::Mutex::new(Some(startup.clone())),
            std::sync::Mutex::new(startup.snapshot),
        ))
        .manage(std::sync::Arc::new(
            engine_commands::EngineManager::default(),
        ))
        .invoke_handler(tauri::generate_handler![
            cloud::cloud_status,
            cloud::download_cloud_files,
            cloud::cancel_cloud_download,
            commands::open_database,
            commands::create_database,
            commands::save_game,
            commands::replace_game,
            commands::set_deleted,
            commands::search_games,
            commands::cancel_search,
            commands::clear_search,
            commands::read_search_page,
            commands::find_position,
            commands::search_position,
            commands::create_search_booster,
            commands::delete_search_booster,
            commands::create_opening_tree,
            commands::cancel_opening_tree,
            commands::delete_opening_tree,
            commands::opening_tree,
            commands::filtered_opening_tree,
            commands::cancel_filtered_opening_tree,
            commands::reference_search,
            commands::reference_statistics,
            commands::cancel_reference,
            commands::read_reference_page,
            commands::read_page,
            commands::read_rows,
            commands::find_row,
            commands::sort_list,
            commands::cancel_sort,
            commands::name_list,
            commands::cancel_name_list,
            commands::read_names,
            commands::find_name,
            commands::crosstable,
            commands::player_statistics,
            commands::cancel_player_statistics,
            commands::statistics_games,
            commands::prepare_against,
            commands::cancel_preparation,
            commands::close_preparation,
            commands::prepared_tree,
            commands::preparation_games,
            commands::player_dossier,
            commands::cancel_dossier,
            commands::tournament_report,
            commands::show_entries,
            playing_style::playing_style,
            playing_style::cancel_playing_style,
            playing_style::validate_playing_style,
            playing_style::load_style_entry,
            error_report::error_report,
            error_report::cancel_error_report,
            error_report::selection_players,
            centipawn::centipawn_analysis,
            centipawn::cancel_centipawn_analysis,
            evaluation_profile::instant_analysis,
            centipawn_list::centipawn_list,
            centipawn_list::cancel_centipawn_list,
            centipawn_list::save_csv,
            evaluation_profile::cancel_instant_analysis,
            repertoire::frequent_players,
            repertoire::generate_repertoire,
            repertoire::repertoire_report,
            repertoire::cancel_repertoire,
            repertoire_workspace::repertoire_snapshot,
            repertoire_workspace::repertoire_edit,
            repertoire_workspace::repertoire_grade,
            replay_training::replay_training_progress,
            replay_training::replay_training_record,
            commands::copy_rows,
            commands::clip_size,
            commands::paste_games,
            commands::cancel_paste,
            commands::export_rows_pgn,
            commands::cancel_export,
            commands::convert_estimate,
            commands::convert_database,
            commands::cancel_convert,
            commands::mark_rows,
            commands::set_names_of_rows,
            commands::check_integrity,
            commands::find_double_games,
            openings::classify_game,
            openings::set_eco_codes,
            openings::opening_report,
            commands::remove_deleted_games,
            commands::cancel_maintenance,
            commands::delete_rows,
            commands::load_entry,
            commands::position_at,
            commands::close_database,
            commands::close_document,
            commands::open_pgn,
            commands::load_pgn_game,
            commands::close_pgn,
            commands::edit_document,
            commands::new_document,
            commands::paste_game,
            commands::document_pgn,
            commands::save_document,
            commands::finish_close,
            commands::startup_request,
            commands::startup_shown,
            engine_commands::probe_engine,
            tablebases::probe_tablebase,
            tablebases::tablebase_annotation,
            blunder_check::blunder_check,
            blunder_check::cancel_blunder_check,
            deep_analysis::deep_analysis,
            deep_analysis::stop_deep_analysis,
            analysis_jobs::analysis_jobs,
            analysis_jobs::add_analysis_jobs,
            analysis_jobs::add_analysis_job,
            analysis_jobs::edit_analysis_job,
            analysis_jobs::clear_analysis_jobs,
            analysis_jobs::run_analysis_jobs,
            analysis_jobs::stop_analysis_jobs,
            settings::get_settings,
            library::start_screen,
            library::folder_databases,
            library::pin_database,
            library::unpin_database,
            library::set_database_kind,
            library::set_database_properties,
            library::set_database_view,
            recovery::begin_recovery,
            recovery::cancel_recovery,
            recovery::recover_database,
            library::forget_recent,
            library::add_folder_shortcut,
            library::remove_folder_shortcut,
            settings::set_settings,
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
        .expect("Cannot build OpenFile")
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
