pub mod commands;
pub mod documents;
pub mod registry;
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(std::sync::Arc::new(registry::Registry::default()))
        .invoke_handler(tauri::generate_handler![
            commands::open_database,
            commands::read_page,
            commands::load_entry,
            commands::position_at,
            commands::close_database,
            commands::close_document
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
        .run(|_, _| {});
}
