use engine::{
    EngineOption, OptionKind, Score, UciResponse, parse_line, position_command, validate_option,
};
fn info(s: &str) -> engine::AnalysisInfo {
    let UciResponse::Info(i) = parse_line(s).unwrap().unwrap() else {
        panic!("Expected info")
    };
    i
}
#[test]
fn score_bound_and_multipv_are_preserved() {
    let i =
        info("info depth 18 multipv 2 score cp -31 lowerbound nodes 9007199254740993 pv e2e4 e7e5");
    assert_eq!(i.multipv, 2);
    assert_eq!(i.nodes, Some(9_007_199_254_740_993));
    assert_eq!(i.depth, Some(18));
    assert_eq!(
        i.score,
        Some(Score::Cp {
            value: -31,
            bound: Some("lowerbound".into())
        })
    );
    assert_eq!(i.pv, Some(vec!["e2e4".into(), "e7e5".into()]));
}
#[test]
fn reordered_fields_unknown_extensions_and_mate_bounds() {
    let i = info(
        "info mystery extension nodes 18446744073709551615 score mate -3 upperbound multipv 4 pv a7a8n h8g7 depth 22",
    );
    assert_eq!(i.nodes, Some(u64::MAX));
    assert_eq!(i.depth, Some(22));
    assert_eq!(i.multipv, 4);
    assert_eq!(
        i.score,
        Some(Score::Mate {
            moves: -3,
            bound: Some("upperbound".into())
        })
    );
    assert_eq!(i.pv, Some(vec!["a7a8n".into(), "h8g7".into()]));
    assert!(
        info("info string depth banana score cp infinity pv invalid")
            .score
            .is_none()
    );
    assert_eq!(
        info("info seldepth 25 time 9 hashfull 8 nps 100 depth 12").depth,
        Some(12)
    );
}
#[test]
fn malformed_known_fields_and_long_lines_are_errors() {
    for s in [
        "info depth",
        "info depth nope",
        "info nodes 18446744073709551616",
        "info multipv 0",
        "info score",
        "info score cp",
        "info score mate nope",
        "bestmove nope",
        "bestmove e2e4 ponder",
        "option name Threads type spin min nope",
        "info pv nope",
    ] {
        assert!(parse_line(s).is_err(), "{s}");
    }
    assert!(parse_line(&"x".repeat(16385)).is_err());
    assert!(parse_line("info depth 3\nquit").is_err());
    assert!(parse_line("debug arbitrary output").unwrap().is_none());
}
#[test]
fn identity_handshake_and_bestmove_are_typed() {
    assert_eq!(
        parse_line(" id name Example Engine ").unwrap(),
        Some(UciResponse::Id {
            field: "name".into(),
            value: "Example Engine".into()
        })
    );
    assert_eq!(parse_line("uciok").unwrap(), Some(UciResponse::UciOk));
    assert_eq!(parse_line("readyok").unwrap(), Some(UciResponse::ReadyOk));
    assert_eq!(
        parse_line("bestmove e2e4 ponder e7e5").unwrap(),
        Some(UciResponse::BestMove {
            best: "e2e4".into(),
            ponder: Some("e7e5".into())
        })
    );
    assert!(parse_line("bestmove (none)").is_ok());
    assert!(parse_line("bestmove 0000").is_ok());
}
fn option(line: &str) -> EngineOption {
    let UciResponse::Option(o) = parse_line(line).unwrap().unwrap() else {
        panic!("Expected option")
    };
    o
}
#[test]
fn multiword_option_names_and_values_are_preserved() {
    let o = option(
        "option name Playing Style type combo default Very Solid var Very Solid var Normal Play",
    );
    assert_eq!(o.name, "Playing Style");
    assert_eq!(o.kind, OptionKind::Combo);
    assert_eq!(o.default.as_deref(), Some("Very Solid"));
    assert_eq!(o.vars, vec!["Very Solid", "Normal Play"]);
    assert_eq!(
        validate_option(&o, "Normal Play").unwrap(),
        "setoption name Playing Style value Normal Play"
    );
    assert!(validate_option(&o, "unknown").is_err());
    let s = option("option name Path To Book type string default /some path/file");
    assert_eq!(s.default.as_deref(), Some("/some path/file"));
    assert!(validate_option(&s, "/other path/file").is_ok());
}
#[test]
fn option_types_validate_values_and_reject_command_injection() {
    let check = option("option name Ponder type check default false");
    assert!(validate_option(&check, "true").is_ok());
    assert!(validate_option(&check, "yes").is_err());
    let spin = option("option name Threads type spin default 1 min 1 max 64");
    assert!(validate_option(&spin, "64").is_ok());
    for v in ["0", "65", "1.5", "1\nquit", "1\risready"] {
        assert!(validate_option(&spin, v).is_err());
    }
    let string = option("option name Book File type string default <empty>");
    assert_eq!(string.default.as_deref(), Some(""));
    assert!(validate_option(&string, "").is_ok());
    assert!(validate_option(&string, "x\nquit").is_err());
    let button = option("option name Clear Hash type button");
    assert_eq!(
        validate_option(&button, "").unwrap(),
        "setoption name Clear Hash"
    );
    assert!(validate_option(&button, "true").is_err());
    let mut bad = string;
    bad.name = "Book\nquit".into();
    assert!(validate_option(&bad, "ok").is_err());
}
#[test]
fn position_commands_validate_fen_and_every_move() {
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    assert_eq!(
        position_command(fen, &["e2e4".into(), "e7e5".into()]).unwrap(),
        format!("position fen {fen} moves e2e4 e7e5")
    );
    assert!(position_command("bad", &[]).is_err());
    assert!(position_command(fen, &["e2e5".into()]).is_err());
    assert!(position_command(&format!("{fen}\nquit"), &[]).is_err());
    assert!(position_command(fen, &["e2e4\nquit".into()]).is_err());
}
