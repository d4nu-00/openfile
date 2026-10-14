use cbh_reader::{
    DataStatus, Database, ReadLimits,
    v2::{encode, moves, write::Writer},
};
use chess_core::{ArrowMark, Game, GameLimits, SquareMark};

fn pgn(text: &str) -> Game {
    chess_core::parse_pgn(text.as_bytes(), &GameLimits::default())
        .unwrap()
        .remove(0)
}

const ANNOTATED: &str = r#"[Event "Club Open"]
[Site "Göteborg"]
[Date "2024.05.17"]
[Round "3.2"]
[White "Müller, Jörg"]
[Black "Zeta, Ana"]
[Result "1/2-1/2"]
[WhiteElo "2310"]
[ECO "C42"]
[Annotator "Lasker, Emanuel"]

{Start} 1. e4 $1 e5 2. Nf3 (2. f4 exf4 (2... d5) 3. Nf3) (2. Bc4) Nf6 {Petrov} 3. Nxe5 d6 1/2-1/2"#;

/// Every node in child order with its move path and annotations, so trees
/// numbered differently compare equal.
fn walk(g: &Game) -> Vec<String> {
    let mut out = vec![];
    let mut stack = vec![(0u32, String::new())];
    while let Some((id, path)) = stack.pop() {
        let n = &g.nodes[id as usize];
        let path = format!("{path} {}", n.uci.as_deref().unwrap_or("-"));
        out.push(format!(
            "{path} {:?} {:?} {:?} {:?} {:?}",
            n.comments, n.pre_comments, n.nags, n.squares, n.arrows
        ));
        for &c in n.children.iter().rev() {
            stack.push((c, path.clone()));
        }
    }
    out
}

fn annotated() -> Game {
    let mut g = pgn(ANNOTATED);
    // Text before 2...Nf6, marks after 1.e4.
    let nf6 = g.nodes[3].children[0] as usize;
    g.nodes[nf6].pre_comments.push("Main line".into());
    g.nodes[1].squares.push(SquareMark {
        color: 'G',
        square: "e4".into(),
    });
    g.nodes[1].arrows.push(ArrowMark {
        color: 'R',
        from: "d7".into(),
        to: "d5".into(),
    });
    g
}

#[test]
fn created_database_reads_back() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("New.2cbh");
    let mut w = Writer::create(&path).unwrap();
    let game = annotated();
    let (ordinal, omitted) = w.append(&game).unwrap();
    assert_eq!(ordinal, 1);
    assert!(omitted.notes.is_empty());
    let second =
        pgn("[White \"Zeta, Ana\"]\n[Black \"Müller, Jörg\"]\n[Result \"0-1\"]\n\n1. d4 d5 0-1");
    assert_eq!(w.append(&second).unwrap().0, 2);
    drop(w);
    // ChessBase index files are never written.
    assert!(!path.with_extension("2lcd").exists());

    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    assert_eq!(db.info().entry_count, 2);
    let page = db.page(1, 2).unwrap();
    let e = &page.entries[0];
    assert_eq!(e.white.as_deref(), Some("Müller, Jörg"));
    assert_eq!(e.event.as_deref(), Some("Club Open"));
    assert_eq!(e.site.as_deref(), Some("Göteborg"));
    assert_eq!(e.date.as_deref(), Some("2024.05.17"));
    assert_eq!(e.round.as_deref(), Some("3.2"));
    assert_eq!(e.result.as_deref(), Some("1/2-1/2"));
    assert_eq!(e.white_elo, Some(2310));
    assert_eq!(e.eco.as_deref(), Some("C42"));
    assert_eq!(e.annotator.as_deref(), Some("Lasker, Emanuel"));
    // Names are shared: both games use the same two player records.
    let f = &page.entries[1];
    assert_eq!((f.white_id, f.black_id), (e.black_id, e.white_id));
    assert_eq!(f.result.as_deref(), Some("0-1"));
    assert_eq!(f.event, None);

    let loaded = db.load(1).unwrap();
    assert_eq!(loaded.annotations_status, DataStatus::Available);
    let back = loaded.game.unwrap();
    assert_eq!(walk(&back), walk(&game));
}

#[test]
fn replace_and_delete_keep_the_entry_count() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Edit.2cbh");
    let mut w = Writer::create(&path).unwrap();
    w.append(&annotated()).unwrap();
    drop(w);
    let mut w = Writer::open(&path).unwrap();
    let other = pgn("[White \"New, Player\"]\n[Result \"1-0\"]\n\n1. c4 1-0");
    w.replace(1, &other).unwrap();
    w.set_deleted(1, true).unwrap();
    assert!(w.set_deleted(2, true).is_err());
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    assert_eq!(db.info().entry_count, 1);
    let e = db.load(1).unwrap();
    assert!(e.summary.deleted);
    assert_eq!(e.summary.white.as_deref(), Some("New, Player"));
    assert_eq!(e.game.unwrap().nodes.len(), 2);
    w.set_deleted(1, false).unwrap();
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    assert!(!db.load(1).unwrap().summary.deleted);
}

#[test]
fn existing_files_and_outside_changes_are_refused() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Guard.2cbh");
    std::fs::write(path.with_extension("2lcd"), b"").unwrap();
    assert!(Writer::create(&path).is_err());
    std::fs::remove_file(path.with_extension("2lcd")).unwrap();
    assert!(Writer::create(&d.path().join("Classic.cbh")).is_err());
    let mut w = Writer::create(&path).unwrap();
    w.append(&annotated()).unwrap();
    // Another program appends to the move file.
    let mut bytes = std::fs::read(path.with_extension("2cbg")).unwrap();
    bytes.extend([0; 16]);
    std::fs::write(path.with_extension("2cbg"), bytes).unwrap();
    assert!(matches!(
        w.append(&annotated()),
        Err(cbh_reader::CbhError::SourceChanged)
    ));
}

#[test]
fn moves_reencode_to_the_same_tree() {
    for text in [
        ANNOTATED,
        "[FEN \"4k3/P7/8/8/8/8/8/4K2R w K - 0 40\"]\n[SetUp \"1\"]\n\n40. a8=Q+ Kd7 41. O-O 1-0",
        "1. e4 d5 2. exd5 c5 3. dxc6 Nxc6 4. Bb5 Qd7 5. Bxc6 Qxc6 *",
    ] {
        let game = pgn(text);
        let bytes = encode::encode_moves(&game).unwrap();
        let back = moves::decode_moves(&bytes, &ReadLimits::default())
            .unwrap()
            .game;
        assert_eq!(back.start_fen, game.start_fen);
        // Moves only: the move encoding carries no annotations.
        let paths = |g: &Game| -> Vec<String> {
            walk(g)
                .iter()
                .map(|l| l.split(" [").next().unwrap().to_string())
                .collect()
        };
        assert_eq!(paths(&back), paths(&game));
    }
}

#[test]
fn unstorable_annotations_are_reported() {
    let mut g = pgn("1. e4 e5 *");
    g.nodes[1].nags = vec![1, 2, 3, 4];
    g.nodes[1].squares.push(SquareMark {
        color: 'X',
        square: "e4".into(),
    });
    let (_, omitted) = encode::encode_annotations(&g);
    assert_eq!(omitted.notes.len(), 2);
}

#[test]
fn one_symbol_per_slot_is_storable() {
    // Move, position and prefix symbols: only move, unclear, better is.
    let mut g = pgn("1. e4 e5 *");
    g.nodes[1].nags = vec![8, 13, 142];
    assert!(encode::encode_annotations(&g).1.notes.is_empty());
    // Zugzwang is a move symbol, as in ChessBase's "!" popup; beside an
    // evaluation and a prefix it still fits.
    g.nodes[1].nags = vec![22, 13, 142];
    assert!(encode::encode_annotations(&g).1.notes.is_empty());
}

#[test]
fn forfeits_and_double_zero_keep_their_code_while_the_result_agrees() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Results.2cbh");
    let mut w = Writer::create(&path).unwrap();
    let with = |result: &str, kept: &str| {
        let mut g = pgn(&format!("[Result \"{result}\"]\n\n1. e4 {result}"));
        g.tags.insert(cbh_reader::RESULT_TAG.into(), kept.into());
        g
    };
    for g in [
        with("1-0", "+:-"),
        with("1/2-1/2", "=:="),
        with("0-1", "-:+"),
        with("*", "0-0"),
        // Edited since: the PGN result wins.
        with("0-1", "+:-"),
    ] {
        w.append(&g).unwrap();
    }
    drop(w);
    let mut db = Database::open(&path, ReadLimits::default()).unwrap();
    let shown: Vec<_> = db
        .page(1, 5)
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.result.unwrap())
        .collect();
    assert_eq!(shown, ["+:-", "=:=", "-:+", "0-0", "0-1"]);
    // Loading keeps the code in the tag, so a copy stores it again.
    let loaded = db.load(1).unwrap().game.unwrap();
    assert_eq!(loaded.tags[cbh_reader::RESULT_TAG], "+:-");
    assert_eq!(loaded.tags["Result"], "1-0");
    assert!(
        !db.load(5)
            .unwrap()
            .game
            .unwrap()
            .tags
            .contains_key(cbh_reader::RESULT_TAG)
    );
}
