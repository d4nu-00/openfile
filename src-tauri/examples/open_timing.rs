//! Times what opening a game costs in the backend: loading the entry,
//! building its view, serialising the response, the first position, and the
//! book and reference queries for a few positions of the game.
//!
//! `cargo run --release -p openfile --example open_timing -- <db> <data dir> [sample]`
//!
//! `<data dir>` is the app's data folder (it holds `search-boosters/`,
//! `opening-trees/` …). `sample` entries are spread evenly over the database.
use openfile::{registry::Registry, trees::Line};
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("database path");
    let data = std::path::PathBuf::from(args.next().expect("data dir"));
    let sample: u64 = args.next().map(|s| s.parse().unwrap()).unwrap_or(400);
    let r = Registry::default();
    r.set_index_dir(data.join("search-boosters"));
    let t = Instant::now();
    let db = r.open_database(path.into(), "o".into()).unwrap();
    println!("open {:.0} ms, {} entries", ms(t), db.entry_count);
    let id = db.database_id;
    let count: u64 = db.entry_count.parse().unwrap();
    let mut rows = vec![];
    for i in 0..sample {
        let ordinal = 1 + i * (count / sample);
        let t = Instant::now();
        let g = r
            .load_entry(id.clone(), ordinal.to_string(), "l".into())
            .unwrap();
        let load = ms(t);
        let Some(doc) = g.document_id.clone() else {
            continue;
        };
        let nodes = g.game.as_ref().map_or(0, |v| v.nodes.len());
        let t = Instant::now();
        let json = serde_json::to_string(&g).unwrap();
        let ser = ms(t);
        let t = Instant::now();
        r.position_at(doc.clone(), 0, "p".into()).unwrap();
        let pos = ms(t);
        rows.push((ordinal, nodes, json.len(), load, ser, pos));
        r.close_document(doc).unwrap();
    }
    rows.sort_by(|a, b| b.3.total_cmp(&a.3));
    let total: f64 = rows.iter().map(|r| r.3).sum();
    println!(
        "{} games: load mean {:.2} ms; slowest:",
        rows.len(),
        total / rows.len() as f64
    );
    for (o, n, bytes, load, ser, pos) in rows.iter().take(8) {
        println!(
            "  entry {o}: {n} nodes, {bytes} B json, load {load:.1} ms, json {ser:.2} ms, position {pos:.2} ms"
        );
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.1));
    for (o, n, bytes, load, ser, pos) in rows.iter().take(4) {
        println!(
            "  largest entry {o}: {n} nodes, {bytes} B json, load {load:.1} ms, json {ser:.2} ms, position {pos:.2} ms"
        );
    }

    // Book and reference for positions along the main line of one game.
    let probe = rows.first().map(|r| r.0).unwrap_or(1);
    let g = r
        .load_entry(id.clone(), probe.to_string(), "l".into())
        .unwrap();
    let view = g.game.unwrap();
    let mut moves = vec![];
    let mut node = 0usize;
    for ply in 0..=60 {
        if [0, 4, 10, 16, 24, 40, 60].contains(&ply) {
            let line = Line {
                start_fen: view.start_fen.clone(),
                moves: moves.clone(),
            };
            let t = Instant::now();
            let tree = r
                .opening_tree(id.clone(), line.clone(), "t".into())
                .unwrap();
            let book = ms(t);
            let t = Instant::now();
            let reference = r.reference_search(id.clone(), line, "r".into()).unwrap();
            println!(
                "ply {ply}: book {book:.1} ms ({:?}, {} games); reference {:.1} ms ({:?}, {} hits)",
                tree.status,
                tree.games,
                ms(t),
                reference.source,
                reference.hits
            );
        }
        let Some(&next) = view.nodes[node].children.first() else {
            break;
        };
        node = next as usize;
        moves.push(view.nodes[node].uci.clone().unwrap());
    }
}
