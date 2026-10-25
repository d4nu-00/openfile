use openfile::{
    clipboard::RowRange,
    registry::Registry,
    srs::{
        deck::{DeckOptions, Grade, Kind, NewOrder, TraineeChoice},
        store::SrsStore,
    },
};
use std::path::Path;

/// 2023-11-14T10:00:00Z.
const NOW: i64 = 1_699_956_000_000;
const MINUTE: i64 = 60_000;
const DAY: i64 = 86_400_000;

const BACK_RANK_WHITE: &str = "6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1";
const BACK_RANK_BLACK: &str = "r5k1/5ppp/8/8/8/8/5PPP/6K1 b - - 0 1";
const ROOK_IN_TWO: &str = "8/8/8/8/7R/4K3/8/6k1 w - - 0 1";

fn puzzle(white: &str, fen: &str, moves: &str) -> String {
    format!(
        "[Event \"Puzzles\"]\n[White \"{white}\"]\n[Black \"Mate\"]\n[Result \"*\"]\n[FEN \"{fen}\"]\n[SetUp \"1\"]\n\n{moves} *\n\n"
    )
}

fn puzzles() -> String {
    puzzle("One", BACK_RANK_WHITE, "1. Ra8#")
        + &puzzle("Two", BACK_RANK_BLACK, "1... Ra1#")
        + &puzzle("Three", ROOK_IN_TWO, "1. Kf3 Kf1 2. Rh1#")
}

const REPERTOIRE: &str = "[Event \"Repertoire\"]\n[White \"Me\"]\n[Black \"Them\"]\n[Result \"*\"]\n\n1. e4 e5 (1... c5 2. Nf3 d6) 2. Nf3 Nc6 (2... d6 3. d4) 3. Bb5 *\n";

fn white() -> DeckOptions {
    DeckOptions {
        trainee: TraineeChoice::White,
        ..DeckOptions::default()
    }
}

fn store(dir: &Path) -> SrsStore {
    let store = SrsStore::default();
    store.set_dir(dir.join("srs"));
    store
}

fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn deck_file(dir: &Path, id: &str) -> std::path::PathBuf {
    dir.join("srs").join("decks").join(format!("{id}.json"))
}

#[test]
fn a_deck_from_a_pgn_of_puzzles_has_one_card_per_game_and_is_stored_in_the_app_data() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let path = write(dir.path(), "Puzzles.pgn", &puzzles());
    let made = s
        .create_from_pgn(path.clone(), "Mate".into(), DeckOptions::default(), NOW, 0)
        .unwrap();
    assert_eq!(made.deck.name, "Mate");
    assert_eq!(made.deck.cards, 3);
    assert_eq!(made.deck.stats.new, 3);
    assert_eq!(made.deck.source, path.to_string_lossy());
    assert_eq!((made.skipped.games, made.skipped.cards_over_limit), (0, 0));
    assert!(deck_file(dir.path(), &made.deck.id).is_file());
    // The source is only read.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), puzzles());
    let detail = s.deck(&made.deck.id).unwrap();
    assert_eq!(
        detail
            .cards
            .iter()
            .map(|c| c.source_label.as_str())
            .collect::<Vec<_>>(),
        [
            "Puzzles.pgn \u{b7} game 1",
            "Puzzles.pgn \u{b7} game 2",
            "Puzzles.pgn \u{b7} game 3"
        ]
    );
}

#[test]
fn a_game_that_does_not_parse_is_skipped_and_counted() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let text = puzzle("One", BACK_RANK_WHITE, "1. Ra8#")
        + "[Event \"Bad\"]\n[Result \"*\"]\n\n1. e4 e5 2. Qh8 *\n\n"
        + &puzzle("Two", BACK_RANK_BLACK, "1... Ra1#");
    let path = write(dir.path(), "Mixed.pgn", &text);
    let made = s
        .create_from_pgn(path, "Mixed".into(), DeckOptions::default(), NOW, 0)
        .unwrap();
    assert_eq!(made.deck.cards, 2);
    assert_eq!(made.skipped.games, 1);
}

#[test]
fn a_repertoire_with_variations_follows_the_options() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let path = write(dir.path(), "Rep.pgn", REPERTOIRE);
    let all = s
        .create_from_pgn(path.clone(), "All".into(), white(), NOW, 0)
        .unwrap();
    assert_eq!(all.deck.cards, 3);
    let main = s
        .create_from_pgn(
            path,
            "Main".into(),
            DeckOptions {
                include_variations: false,
                ..white()
            },
            NOW,
            0,
        )
        .unwrap();
    assert_eq!(main.deck.cards, 1);
    // Both decks are listed.
    let listing = s.decks(NOW, 0).unwrap();
    assert_eq!(listing.decks.len(), 2);
    assert!(listing.unreadable.is_empty());
}

#[test]
fn creating_refuses_bad_names_and_options() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let path = write(dir.path(), "P.pgn", &puzzles());
    assert!(
        s.create_from_pgn(path.clone(), "  ".into(), DeckOptions::default(), NOW, 0)
            .is_err()
    );
    let bad = DeckOptions {
        learning_steps: vec![],
        ..DeckOptions::default()
    };
    assert!(s.create_from_pgn(path, "x".into(), bad, NOW, 0).is_err());
    assert!(s.decks(NOW, 0).unwrap().decks.is_empty());
}

#[test]
fn the_queue_lists_new_cards_in_source_order_with_the_first_cards_data() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let made = s
        .create_from_pgn(
            write(dir.path(), "P.pgn", &puzzles()),
            "Mate".into(),
            DeckOptions::default(),
            NOW,
            0,
        )
        .unwrap();
    let id = &made.deck.id;
    let detail = s.deck(id).unwrap();
    let ids: Vec<&str> = detail.cards.iter().map(|c| c.id.as_str()).collect();
    let got = s.queue(id, NOW, 0).unwrap();
    assert_eq!(got.queue.cards, ids);
    assert_eq!((got.queue.new_due, got.queue.new_left), (3, 20));
    let next = got.next.unwrap();
    assert_eq!(next.card.id, ids[0]);
    // New cards are tried first unless the deck shows them first.
    assert!(!next.learn);
    assert_eq!(next.mate_in, Some(1));
    assert_eq!(next.plies.len(), 1);
    assert_eq!(next.plies[0].fen, BACK_RANK_WHITE);
    assert!(next.plies[0].legal_uci.contains(&"a1a8".to_string()));
    assert!(next.plies[0].other_moves.is_empty());
    assert!(next.end_fen.starts_with("R5k1/"));
    // What each grade would do, for a new card.
    let due: Vec<(Grade, i64)> = next
        .previews
        .iter()
        .map(|p| (p.grade, p.due_in_ms))
        .collect();
    assert_eq!(
        due,
        [
            (Grade::Again, MINUTE),
            (Grade::Hard, MINUTE),
            (Grade::Good, 10 * MINUTE),
            (Grade::Easy, 4 * DAY)
        ]
    );
    // A black puzzle starts with Black to move; a two-move line has the opponent's reply.
    s.suspend(id, ids[0], true).unwrap();
    let second = s.queue(id, NOW, 0).unwrap().next.unwrap();
    assert_eq!(second.card.id, ids[1]);
    assert_eq!(second.card.trainee, openfile::srs::deck::Colour::Black);
    s.suspend(id, ids[1], true).unwrap();
    let third = s.queue(id, NOW, 0).unwrap().next.unwrap();
    assert_eq!(third.plies.len(), 3);
    assert_eq!(third.mate_in, Some(2));
    assert_eq!(third.plies[1].fen, "8/8/8/8/7R/5K2/8/6k1 b - - 1 1");
    assert!(third.plies[0].legal_uci.contains(&"e3f3".to_string()));
    // The opponent's ply needs no legal moves.
    assert!(third.plies[1].legal_uci.is_empty());
    assert!(third.plies[2].legal_uci.contains(&"h4h1".to_string()));
}

#[test]
fn a_deck_that_shows_new_cards_first_makes_them_lessons() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let made = s
        .create_from_pgn(
            write(dir.path(), "P.pgn", &puzzles()),
            "Mate".into(),
            DeckOptions {
                show_new_first: true,
                ..DeckOptions::default()
            },
            NOW,
            0,
        )
        .unwrap();
    assert!(s.queue(&made.deck.id, NOW, 0).unwrap().next.unwrap().learn);
    // A repertoire line does not end in mate.
    let rep = s
        .create_from_pgn(
            write(dir.path(), "R.pgn", REPERTOIRE),
            "Rep".into(),
            white(),
            NOW,
            0,
        )
        .unwrap();
    assert_eq!(
        s.queue(&rep.deck.id, NOW, 0).unwrap().next.unwrap().mate_in,
        None
    );
}

#[test]
fn moves_that_other_lines_of_the_deck_play_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let text = "[White \"Me\"]\n[Black \"Them\"]\n\n1. e4 (1. d4 d5 2. c4) e5 2. Nf3 *\n";
    let made = s
        .create_from_pgn(
            write(dir.path(), "R.pgn", text),
            "R".into(),
            white(),
            NOW,
            0,
        )
        .unwrap();
    assert_eq!(made.deck.cards, 2);
    let next = s.queue(&made.deck.id, NOW, 0).unwrap().next.unwrap();
    assert_eq!(next.card.moves[0].uci, "e2e4");
    assert_eq!(next.plies[0].other_moves, ["d2d4"]);
    // After 1. e4 e5 no other line plays here.
    assert!(next.plies[2].other_moves.is_empty());
}

#[test]
fn answers_move_the_states_and_persist_across_a_new_store_instance() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let made = s
        .create_from_pgn(
            write(dir.path(), "P.pgn", &puzzles()),
            "Mate".into(),
            DeckOptions::default(),
            NOW,
            0,
        )
        .unwrap();
    let id = made.deck.id.clone();
    let cards = s.queue(&id, NOW, 0).unwrap().queue.cards;
    // Good on a new card: the second learning step, back in 10 minutes.
    let done = s
        .answer(&id, &cards[0], Grade::Good, 0, 0, 1500, NOW, 0)
        .unwrap();
    assert_eq!(
        (done.state.kind, done.state.step, done.state.due),
        (Kind::Learning, 1, NOW + 10 * MINUTE)
    );
    assert_eq!(done.session.queue.cards, cards[1..]);
    assert_eq!(done.session.stats.learning, 1);
    // A mistake always ends in Again, whatever the grade sent.
    let wrong = s
        .answer(&id, &cards[1], Grade::Easy, 2, 0, 900, NOW, 0)
        .unwrap();
    assert_eq!(
        (wrong.state.kind, wrong.state.step, wrong.state.due),
        (Kind::Learning, 0, NOW + MINUTE)
    );
    // A hint caps the grade at Hard.
    let hint = s
        .answer(&id, &cards[2], Grade::Good, 0, 1, 900, NOW, 0)
        .unwrap();
    assert_eq!((hint.state.step, hint.state.due), (0, NOW + MINUTE));
    // A new store instance reads the same progress.
    let again = store(dir.path());
    let detail = again.deck(&id).unwrap();
    let steps: Vec<(Kind, u32)> = detail
        .cards
        .iter()
        .map(|c| (c.state.kind, c.state.step))
        .collect();
    assert_eq!(
        steps,
        [
            (Kind::Learning, 1),
            (Kind::Learning, 0),
            (Kind::Learning, 0)
        ]
    );
    // Nothing is due now; the learning cards are shown ahead of time.
    let queue = again.queue(&id, NOW, 0).unwrap().queue;
    assert_eq!(queue.cards.len(), 3);
    assert!(queue.ahead);
    // A minute later two of them are due; the third is ten minutes away.
    let later = again.queue(&id, NOW + MINUTE, 0).unwrap().queue;
    assert_eq!(later.cards.len(), 2);
    assert!(!later.ahead);
    assert_eq!(later.learning_due, 2);
    // The log kept the answers with their mistakes and hints.
    let text = std::fs::read_to_string(deck_file(dir.path(), &id)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let log = value["log"].as_array().unwrap();
    assert_eq!(log.len(), 3);
    assert_eq!(log[1]["mistakes"], 2);
    assert_eq!(log[1]["grade"], "again");
    assert_eq!(log[2]["hints"], 1);
    assert_eq!(log[2]["grade"], "hard");
}

#[test]
fn the_answer_of_a_card_that_is_not_in_the_deck_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let made = s
        .create_from_pgn(
            write(dir.path(), "P.pgn", &puzzles()),
            "Mate".into(),
            DeckOptions::default(),
            NOW,
            0,
        )
        .unwrap();
    assert!(
        s.answer(&made.deck.id, "nope", Grade::Good, 0, 0, 1, NOW, 0)
            .is_err()
    );
}

#[test]
fn an_update_keeps_progress_adds_new_lines_and_suspends_removed_ones() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let one = puzzle("One", BACK_RANK_WHITE, "1. Ra8#");
    let two = puzzle("Two", BACK_RANK_BLACK, "1... Ra1#");
    let three = puzzle("Three", ROOK_IN_TWO, "1. Kf3 Kf1 2. Rh1#");
    let path = write(dir.path(), "P.pgn", &(one.clone() + &two));
    let made = s
        .create_from_pgn(path.clone(), "Mate".into(), DeckOptions::default(), NOW, 0)
        .unwrap();
    let id = made.deck.id.clone();
    let cards = s.queue(&id, NOW, 0).unwrap().queue.cards;
    s.answer(&id, &cards[0], Grade::Good, 0, 0, 1000, NOW, 0)
        .unwrap();
    // The first puzzle is gone and a new one is added.
    std::fs::write(&path, two.clone() + &three).unwrap();
    let updated = s.update_from_pgn(&id, path.clone(), NOW, 0).unwrap();
    assert_eq!(
        (
            updated.added,
            updated.kept,
            updated.suspended,
            updated.restored
        ),
        (1, 1, 1, 0)
    );
    let detail = s.deck(&id).unwrap();
    assert_eq!(detail.cards.len(), 3);
    let first = detail.cards.iter().find(|c| c.id == cards[0]).unwrap();
    assert!(first.suspended && first.removed);
    // Its progress is kept.
    assert_eq!(first.state.kind, Kind::Learning);
    assert_eq!(updated.deck.stats.suspended, 1);
    // The suspended line is not studied; the others are in source order.
    let queue = s.queue(&id, NOW, 0).unwrap().queue;
    assert_eq!(queue.cards.len(), 2);
    assert_eq!(queue.cards[0], cards[1]);
    // It comes back with its progress when the source has it again.
    std::fs::write(&path, one + &two + &three).unwrap();
    let back = s.update_from_pgn(&id, path, NOW, 0).unwrap();
    assert_eq!(
        (back.added, back.kept, back.suspended, back.restored),
        (0, 3, 0, 1)
    );
    let detail = s.deck(&id).unwrap();
    let first = detail.cards.iter().find(|c| c.id == cards[0]).unwrap();
    assert!(!first.suspended && !first.removed);
    assert_eq!(first.state.kind, Kind::Learning);
    // A card the user suspended stays suspended through an update.
    s.suspend(&id, &cards[1], true).unwrap();
    let path = dir.path().join("P.pgn");
    let kept = s.update_from_pgn(&id, path, NOW, 0).unwrap();
    assert_eq!(kept.deck.stats.suspended, 1);
}

#[test]
fn options_names_and_deleting() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let made = s
        .create_from_pgn(
            write(dir.path(), "P.pgn", &puzzles()),
            "Mate".into(),
            DeckOptions::default(),
            NOW,
            0,
        )
        .unwrap();
    let id = made.deck.id;
    let options = DeckOptions {
        new_per_day: 1,
        new_order: NewOrder::Random,
        ..DeckOptions::default()
    };
    s.set_options(&id, options.clone()).unwrap();
    assert_eq!(s.deck(&id).unwrap().options, options);
    assert_eq!(s.queue(&id, NOW, 0).unwrap().queue.cards.len(), 1);
    assert!(
        s.set_options(
            &id,
            DeckOptions {
                graduating_interval: 0.0,
                ..DeckOptions::default()
            }
        )
        .is_err()
    );
    assert_eq!(s.deck(&id).unwrap().options, options);
    s.rename(&id, " Mates in one ".into()).unwrap();
    assert_eq!(s.deck(&id).unwrap().name, "Mates in one");
    assert!(s.rename(&id, "".into()).is_err());
    s.delete(&id).unwrap();
    assert!(!deck_file(dir.path(), &id).exists());
    assert!(s.deck(&id).is_err());
    assert!(s.decks(NOW, 0).unwrap().decks.is_empty());
}

#[test]
fn ids_are_not_paths() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    for id in ["../x", "a/b", "", "A.json", "x\\y"] {
        assert!(s.deck(id).is_err(), "{id}");
        assert!(s.delete(id).is_err(), "{id}");
    }
}

#[test]
fn unreadable_and_newer_deck_files_are_reported_and_never_written() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(dir.path());
    let made = s
        .create_from_pgn(
            write(dir.path(), "P.pgn", &puzzles()),
            "Mate".into(),
            DeckOptions::default(),
            NOW,
            0,
        )
        .unwrap();
    let decks = dir.path().join("srs").join("decks");
    std::fs::write(decks.join("broken.json"), "{").unwrap();
    let newer = r#"{"version":2,"id":"future","name":"Future","unknown":[1,2,3]}"#;
    std::fs::write(decks.join("future.json"), newer).unwrap();
    std::fs::write(decks.join("notes.txt"), "not a deck").unwrap();
    let listing = s.decks(NOW, 0).unwrap();
    assert_eq!(listing.decks.len(), 1);
    assert_eq!(listing.decks[0].id, made.deck.id);
    let mut files: Vec<(&str, &str)> = listing
        .unreadable
        .iter()
        .map(|u| (u.file.as_str(), u.reason.as_str()))
        .collect();
    files.sort();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].0, "broken.json");
    assert_eq!(files[1].0, "future.json");
    assert!(files[1].1.contains("newer"), "{}", files[1].1);
    // Every command that would write refuses, and the files stay as they were.
    for id in ["broken", "future"] {
        assert!(s.rename(id, "x".into()).is_err());
        assert!(s.set_options(id, DeckOptions::default()).is_err());
        assert!(s.suspend(id, "c", true).is_err());
        assert!(s.answer(id, "c", Grade::Good, 0, 0, 1, NOW, 0).is_err());
        assert!(s.queue(id, NOW, 0).is_err());
        assert!(
            s.update_from_pgn(id, dir.path().join("P.pgn"), NOW, 0)
                .is_err()
        );
    }
    assert_eq!(
        std::fs::read_to_string(decks.join("broken.json")).unwrap(),
        "{"
    );
    assert_eq!(
        std::fs::read_to_string(decks.join("future.json")).unwrap(),
        newer
    );
    // No temporary files are left behind.
    let mut names: Vec<String> = std::fs::read_dir(&decks)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 4, "{names:?}");
    // Deleting a broken deck is allowed: it is how the user cleans up.
    s.delete("broken").unwrap();
    assert!(!decks.join("broken.json").exists());
}

fn range(start: u64, end: u64) -> RowRange {
    serde_json::from_value(serde_json::json!({
        "start": start.to_string(),
        "end": end.to_string()
    }))
    .unwrap()
}

/// A database of the puzzles and one ordinary game, built from a PGN.
fn database(r: &Registry, dir: &Path) -> String {
    let db = r
        .create_database(dir.join("Mates.2cbh"), "c".into())
        .unwrap();
    let text =
        puzzles() + "[White \"A\"]\n[Black \"B\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Qh5 1-0\n\n";
    let pgn = write(dir, "g.pgn", &text);
    let c = r.open_pgn(pgn, "o".into()).unwrap();
    for i in 0..4 {
        let doc = r
            .load_pgn_game(c.collection_id.clone(), i, "l".into())
            .unwrap()
            .document_id;
        r.save_game(doc, db.database_id.clone(), "0".into(), false)
            .unwrap();
    }
    db.database_id
}

#[test]
fn a_deck_from_the_rows_of_a_database_list() {
    let dir = tempfile::tempdir().unwrap();
    let r = Registry::default();
    r.set_index_dir(dir.path().join("search-boosters"));
    let db = database(&r, dir.path());
    let s = store(dir.path());
    // Rows 1-3 are the puzzles (the list is in entry order).
    let made = s
        .create_from_rows(
            &r,
            db.clone(),
            vec![range(1, 4)],
            "From list".into(),
            DeckOptions::default(),
            NOW,
            0,
        )
        .unwrap();
    assert_eq!(made.deck.cards, 3);
    assert_eq!(made.deck.source_kind, openfile::srs::deck::SourceKind::List);
    let labels: Vec<String> = s
        .deck(&made.deck.id)
        .unwrap()
        .cards
        .iter()
        .map(|c| c.source_label.clone())
        .collect();
    assert_eq!(
        labels,
        [
            "Mates \u{b7} entry 1",
            "Mates \u{b7} entry 2",
            "Mates \u{b7} entry 3"
        ]
    );
    // Updating from a smaller selection suspends the others.
    let updated = s
        .update_from_rows(&r, &made.deck.id, db.clone(), vec![range(2, 4)], NOW, 0)
        .unwrap();
    assert_eq!((updated.added, updated.kept, updated.suspended), (0, 2, 1));
    // The selection with the ordinary game: its main line is a card for White, who moves first.
    let all = s
        .create_from_rows(
            &r,
            db,
            vec![range(1, 5)],
            "All".into(),
            DeckOptions::default(),
            NOW,
            0,
        )
        .unwrap();
    assert_eq!(all.deck.cards, 4);
}

#[test]
fn rows_of_an_unknown_database_are_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let r = Registry::default();
    let s = store(dir.path());
    assert!(
        s.create_from_rows(
            &r,
            "nope".into(),
            vec![range(1, 2)],
            "x".into(),
            DeckOptions::default(),
            NOW,
            0
        )
        .is_err()
    );
}
