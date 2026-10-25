//! Makes a spaced repetition deck from a PGN file in a temporary app data
//! folder, prints its stats and queue, and answers a few cards at fixed times.
//!
//! `cargo run -p openfile --example srs_deck -- <file.pgn> [app-data-folder]`
use openfile::srs::{
    deck::{DeckOptions, Grade},
    store::SrsStore,
};

/// 2023-11-14T10:00:00Z.
const NOW: i64 = 1_699_956_000_000;
const MINUTE: i64 = 60_000;
const DAY: i64 = 86_400_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let pgn = args.next().ok_or("usage: srs_deck <file.pgn> [folder]")?;
    let temp = tempfile::tempdir()?;
    let folder = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| temp.path().to_path_buf());
    let store = SrsStore::default();
    store.set_dir(folder.join("srs"));
    let made = store
        .create_from_pgn(
            pgn.into(),
            "Mate chains".into(),
            DeckOptions::default(),
            NOW,
            0,
        )
        .map_err(|e| e.message)?;
    let id = made.deck.id.clone();
    println!(
        "deck {:?}: {} cards, skipped games {}, lines over the limit {}",
        made.deck.name, made.deck.cards, made.skipped.games, made.skipped.cards_over_limit
    );
    println!("stats {:?}", made.deck.stats);
    let first = store.queue(&id, NOW, 0).map_err(|e| e.message)?;
    println!(
        "queue: {} cards, new {} (limit left {}), learning {}, reviews {}",
        first.queue.cards.len(),
        first.queue.new_due,
        first.queue.new_left,
        first.queue.learning_due,
        first.queue.review_due
    );
    let detail = store.deck(&id).map_err(|e| e.message)?;
    for (i, card) in first.queue.cards.iter().take(10).enumerate() {
        let row = detail.cards.iter().find(|c| &c.id == card).unwrap();
        println!(
            "  {:>2}. {} {} plies, {}",
            i + 1,
            row.id,
            row.plies,
            row.source_label
        );
    }
    let next = first.next.ok_or("no first card")?;
    let line: Vec<&str> = next.card.moves.iter().map(|m| m.san.as_str()).collect();
    println!(
        "first card: {} {:?} trainee {:?} learn {}",
        next.card.start_fen, line, next.card.trainee, next.learn
    );
    let cards = first.queue.cards.clone();
    let show = |label: &str, state: &openfile::srs::deck::CardState| {
        println!(
            "{label}: {:?} step {} due {:+}min interval {:.2}d ease {:.2} reps {} lapses {}",
            state.kind,
            state.step,
            (state.due - NOW) / MINUTE,
            state.interval,
            state.ease,
            state.reps,
            state.lapses
        );
    };
    let answer = |card: &str, grade, mistakes, hints, at: i64| {
        store
            .answer(&id, card, grade, mistakes, hints, 2000, at, 0)
            .map_err(|e| e.message)
    };
    show(
        "card 1 Good (new)",
        &answer(&cards[0], Grade::Good, 0, 0, NOW)?.state,
    );
    show(
        "card 2 Easy with a mistake",
        &answer(&cards[1], Grade::Easy, 1, 0, NOW)?.state,
    );
    show(
        "card 3 Good with a hint",
        &answer(&cards[2], Grade::Good, 0, 1, NOW)?.state,
    );
    show(
        "card 4 Easy (new)",
        &answer(&cards[3], Grade::Easy, 0, 0, NOW)?.state,
    );
    let after = store.queue(&id, NOW, 0).map_err(|e| e.message)?;
    println!(
        "queue after: {} cards, learning-ahead {}, next {}",
        after.queue.cards.len(),
        after.queue.ahead,
        after.next.map_or("-".into(), |n| n.card.id)
    );
    let r = answer(&cards[0], Grade::Good, 0, 0, NOW + 10 * MINUTE)?;
    show("card 1 Good 10 min later", &r.state);
    let r = answer(&cards[0], Grade::Good, 0, 0, NOW + DAY)?;
    show("card 1 Good a day later", &r.state);
    let r = answer(&cards[0], Grade::Again, 0, 0, NOW + 4 * DAY)?;
    show("card 1 Again four days later", &r.state);
    println!("stats {:?}", r.session.stats);
    Ok(())
}
