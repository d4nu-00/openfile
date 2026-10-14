use chess_workbench::{registry::Registry, settings::Settings};

#[test]
fn opening_records_recent_and_my_databases_and_folders_list_databases() {
    let d = tempfile::tempdir().unwrap();
    let s = Settings::default();
    s.set_dir(&d.path().join("config"));
    let r = Registry::default();
    let dbs = d.path().join("dbs");
    std::fs::create_dir(&dbs).unwrap();
    let a = dbs.join("Alpha.2cbh");
    let b = dbs.join("Beta.2cbh");
    for p in [&a, &b] {
        r.create_database(p.clone(), "c".into()).unwrap();
        s.record_opened(p).unwrap();
    }
    let a = std::fs::canonicalize(&a).unwrap();
    let b = std::fs::canonicalize(&b).unwrap();
    std::fs::write(dbs.join("notes.txt"), "x").unwrap();
    let screen = s.start_screen();
    let names: Vec<_> = screen.recent.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["Beta", "Alpha"]);
    assert_eq!(screen.favorites.len(), 2);
    assert!(screen.favorites.iter().all(|f| f.favorite && f.exists));
    assert_eq!(screen.favorites[0].entries, Some(0));
    assert_eq!(screen.kinds.len(), 20);
    // Opening again moves it first without a second symbol.
    s.record_opened(&a).unwrap();
    let screen = s.start_screen();
    assert_eq!(screen.recent[0].path, a);
    assert_eq!(screen.favorites.len(), 2);

    // Remove Database Symbol keeps the files and the recent entry.
    s.unpin_database(&a).unwrap();
    let screen = s.start_screen();
    assert_eq!(screen.favorites.len(), 1);
    assert!(a.exists());
    assert!(!screen.recent[0].favorite);
    s.pin_database(&a).unwrap();
    s.set_database_kind(&a, "openings").unwrap();
    assert!(s.set_database_kind(&a, "nonsense").is_err());
    assert!(
        s.set_database_kind(&dbs.join("missing.2cbh"), "work")
            .is_err()
    );
    let screen = s.start_screen();
    assert_eq!(
        screen.favorites.iter().find(|f| f.path == a).unwrap().kind,
        "openings"
    );

    // Folder shortcuts list the databases directly in the folder.
    s.add_folder_shortcut(&dbs).unwrap();
    s.add_folder_shortcut(&dbs).unwrap();
    assert!(s.add_folder_shortcut(&d.path().join("nowhere")).is_err());
    assert!(s.add_folder_shortcut("relative".as_ref()).is_err());
    let screen = s.start_screen();
    assert_eq!(screen.folders.len(), 1);
    let listing = s.folder_databases(&dbs).unwrap();
    let names: Vec<_> = listing.symbols.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "Beta"]);
    assert!(!listing.truncated);
    s.remove_folder_shortcut(&dbs).unwrap();
    assert!(s.start_screen().folders.is_empty());

    // A database deleted from disk shows as missing.
    std::fs::remove_file(&b).unwrap();
    let screen = s.start_screen();
    assert!(!screen.recent.iter().find(|x| x.path == b).unwrap().exists);
    s.forget_recent(&b).unwrap();
    assert!(s.start_screen().recent.iter().all(|x| x.path != b));

    // The lists are in the settings file, nowhere near the databases.
    let text = std::fs::read_to_string(d.path().join("config/settings.json")).unwrap();
    assert!(text.contains("recentDatabases") && !text.contains("folderShortcuts"));
    assert!(std::fs::read_dir(&dbs).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .contains("settings")
    }));
}

#[test]
fn lists_are_bounded_and_checked() {
    use chess_workbench::settings::{AppSettings, FavoriteDatabase};
    for bad in [
        AppSettings {
            recent_databases: (0..17).map(|i| format!("/db/{i}.cbh").into()).collect(),
            ..Default::default()
        },
        AppSettings {
            favorites: vec![FavoriteDatabase {
                path: "relative.cbh".into(),
                kind: "work".into(),
            }],
            ..Default::default()
        },
        AppSettings {
            folder_shortcuts: vec!["/a".into(), "/a".into()],
            ..Default::default()
        },
    ] {
        assert!(bad.check().is_err());
    }
    let d = tempfile::tempdir().unwrap();
    let st = Settings::default();
    st.set_dir(d.path());
    for i in 0..20 {
        let p = d.path().join(format!("{i}.cbh"));
        st.record_opened(&p).unwrap();
    }
    assert_eq!(st.start_screen().recent.len(), 16);
    assert_eq!(st.start_screen().favorites.len(), 20);
}
