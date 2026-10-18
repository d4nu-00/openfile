use openfile::settings::{AppSettings, InstalledEngine, Settings};

#[test]
fn settings_default_check_and_round_trip() {
    let d = tempfile::tempdir().unwrap();
    let s = Settings::default();
    s.set_dir(&d.path().join("config"));
    // Nothing saved yet: the reference app's default of 8 game tabs.
    assert_eq!(s.load(), AppSettings::default());
    assert_eq!(s.load().max_game_tabs, 8);

    let engine = InstalledEngine {
        id: "e1".into(),
        name: "Stockfish".into(),
        path: d.path().join("stockfish"),
        options: [("Threads".to_string(), "4".to_string())].into(),
    };
    let wanted = AppSettings {
        max_game_tabs: 1,
        engines: vec![engine.clone()],
        default_engine: Some("e1".into()),
        tablebase_dir: Some(d.path().join("syzygy")),
        ..AppSettings::default()
    };
    s.save(&wanted).unwrap();
    assert_eq!(s.load(), wanted);

    for bad in [
        AppSettings {
            max_game_tabs: 9,
            ..AppSettings::default()
        },
        AppSettings {
            default_engine: Some("missing".into()),
            ..AppSettings::default()
        },
        AppSettings {
            tablebase_dir: Some("relative/syzygy".into()),
            ..AppSettings::default()
        },
        AppSettings {
            engines: vec![engine.clone(), engine.clone()],
            ..AppSettings::default()
        },
        AppSettings {
            engines: vec![InstalledEngine {
                path: "relative/engine".into(),
                ..engine.clone()
            }],
            ..AppSettings::default()
        },
    ] {
        assert!(s.save(&bad).is_err());
    }
    // A damaged file reads as the defaults and is left alone.
    let file = d.path().join("config/settings.json");
    std::fs::write(&file, "{ not json").unwrap();
    assert_eq!(s.load(), AppSettings::default());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ not json");
}

#[test]
fn the_old_folder_moves_once() {
    let d = tempfile::tempdir().unwrap();
    let (old, new) = (d.path().join("old"), d.path().join("new"));
    std::fs::create_dir_all(old.join("opening-trees")).unwrap();
    openfile::move_old_folder(&old, &new);
    assert!(new.join("opening-trees").is_dir() && !old.exists());
    // An existing new folder is never replaced.
    std::fs::create_dir_all(&old).unwrap();
    openfile::move_old_folder(&old, &new);
    assert!(old.exists() && new.join("opening-trees").is_dir());
}
