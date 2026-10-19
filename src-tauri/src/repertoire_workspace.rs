//! Local, position-based White and Black opening choices. No source database
//! or game document is changed by this module.
use crate::{
    commands::AppError,
    trees::{self, Line},
};
use serde::{Deserialize, Serialize};
use shakmaty::{EnPassantMode, Position, fen::Fen, san::SanPlus, uci::UciMove};
use std::{collections::BTreeMap, io::Write, path::PathBuf, sync::Mutex};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    White,
    Black,
}

impl Side {
    fn label(self) -> &'static str {
        if self == Self::White {
            "white"
        } else {
            "black"
        }
    }
    fn owns(self, pos: &shakmaty::Chess) -> bool {
        (self == Self::White) == (pos.turn() == shakmaty::Color::White)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditAction {
    Add,
    Remove,
    Prefer,
    Note,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedMove {
    uci: String,
    preferred: bool,
    note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedPosition {
    key: String,
    side: Side,
    line: Line,
    moves: Vec<SavedMove>,
}

#[derive(Default, Deserialize, Serialize)]
struct RepertoireFile {
    version: u8,
    entries: Vec<SavedPosition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeStatus {
    pub attempts: u32,
    pub correct: u32,
    pub needs_retry: bool,
}

#[derive(Default, Deserialize, Serialize)]
struct PracticeFile {
    version: u8,
    positions: BTreeMap<String, PracticeStatus>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceView {
    pub uci: String,
    pub san: String,
    pub preferred: bool,
    pub note: String,
    pub valid: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionView {
    pub key: String,
    pub fen: String,
    pub line: Line,
    pub route: String,
    pub own_turn: bool,
    pub moves: Vec<ChoiceView>,
    pub practice: PracticeStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub entries: Vec<PositionView>,
    pub current: Option<PositionView>,
    pub legal: Vec<ChoiceView>,
    pub own_turn: bool,
    pub answered_replies: BTreeMap<String, bool>,
    pub reply_positions: BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grade {
    pub correct: bool,
    pub played_san: String,
    pub preferred_san: Option<String>,
    pub practice: PracticeStatus,
}

#[derive(Default)]
pub struct RepertoireWorkspace {
    dir: Mutex<Option<PathBuf>>,
    edit_lock: Mutex<()>,
}

fn error(message: impl Into<String>) -> AppError {
    AppError::new("repertoire", message)
}
fn io(e: impl std::fmt::Display) -> AppError {
    error(e.to_string())
}

fn position_key(pos: &shakmaty::Chess) -> String {
    Fen::from_position(pos, EnPassantMode::Legal)
        .to_string()
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

fn legal_move(pos: &shakmaty::Chess, uci: &str) -> Result<shakmaty::Move, AppError> {
    UciMove::from_ascii(uci.as_bytes())
        .ok()
        .and_then(|m| m.to_move(pos).ok())
        .ok_or_else(|| error(format!("Illegal repertoire move {uci}")))
}

fn route(line: &Line) -> Result<String, AppError> {
    let mut pos = chess_core::model::parse_position(&line.start_fen)?;
    let mut result = Vec::new();
    for uci in &line.moves {
        if uci == "0000" {
            result.push("--".to_string());
            pos = chess_core::model::apply_uci(pos, uci)?;
            continue;
        }
        let mv = legal_move(&pos, uci)?;
        result.push(format!(
            "{}{}",
            if pos.turn() == shakmaty::Color::White {
                format!("{}. ", pos.fullmoves().get())
            } else {
                String::new()
            },
            SanPlus::from_move(pos.clone(), mv)
        ));
        pos.play_unchecked(mv);
    }
    Ok(if result.is_empty() {
        "Start position".into()
    } else {
        result.join(" ")
    })
}

impl RepertoireWorkspace {
    pub fn set_dir(&self, dir: PathBuf) {
        *self.dir.lock().expect("repertoire directory") = Some(dir);
    }
    fn path(&self, name: &str) -> Result<PathBuf, AppError> {
        Ok(self
            .dir
            .lock()
            .map_err(io)?
            .as_ref()
            .ok_or_else(|| error("No app data folder"))?
            .join(name))
    }
    fn read<T: for<'de> Deserialize<'de> + Default>(&self, name: &str) -> Result<T, AppError> {
        let path = self.path(name)?;
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
            Err(e) => Err(io(e)),
        }
    }
    fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<(), AppError> {
        let path = self.path(name)?;
        let dir = path.parent().ok_or_else(|| error("No app data folder"))?;
        std::fs::create_dir_all(dir).map_err(io)?;
        let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(io)?;
        serde_json::to_writer_pretty(&mut temp, value).map_err(io)?;
        temp.flush().map_err(io)?;
        temp.as_file().sync_all().map_err(io)?;
        temp.persist(path).map_err(|e| io(e.error))?;
        Ok(())
    }
    fn files(&self) -> Result<(RepertoireFile, PracticeFile), AppError> {
        let repertoire: RepertoireFile = self.read("repertoire.json")?;
        let practice: PracticeFile = self.read("repertoire-practice.json")?;
        if repertoire.version > 1 || practice.version > 1 {
            return Err(error("Newer repertoire file version"));
        }
        Ok((repertoire, practice))
    }
    fn view(entry: &SavedPosition, progress: PracticeStatus) -> Option<PositionView> {
        let saved = trees::replay(&entry.line)
            .ok()
            .and_then(|(pos, _, _)| (position_key(&pos) == entry.key).then_some(pos));
        let (pos, line, route) = if let Some(pos) = saved {
            (pos, entry.line.clone(), route(&entry.line).ok()?)
        } else {
            let fen = format!("{} 0 1", entry.key);
            let pos = chess_core::model::parse_position(&fen).ok()?;
            if position_key(&pos) != entry.key {
                return None;
            }
            (
                pos,
                Line {
                    start_fen: fen,
                    moves: Vec::new(),
                },
                "Saved position (original route unavailable)".to_string(),
            )
        };
        Some(PositionView {
            key: entry.key.clone(),
            fen: Fen::from_position(&pos, EnPassantMode::Legal).to_string(),
            line,
            route,
            own_turn: entry.side.owns(&pos),
            moves: entry
                .moves
                .iter()
                .map(|m| {
                    let valid = legal_move(&pos, &m.uci).is_ok();
                    ChoiceView {
                        uci: m.uci.clone(),
                        san: if valid {
                            trees::san(&pos, &m.uci)
                        } else {
                            m.uci.clone()
                        },
                        preferred: m.preferred,
                        note: m.note.clone(),
                        valid,
                    }
                })
                .collect(),
            practice: progress,
        })
    }
    pub fn snapshot(&self, line: Line, side: Side) -> Result<Snapshot, AppError> {
        let (pos, _, _) = trees::replay(&line)?;
        let key = position_key(&pos);
        let (file, practice) = self.files()?;
        let own_turn = side.owns(&pos);
        let (answered_replies, reply_positions) = if own_turn {
            (BTreeMap::new(), BTreeMap::new())
        } else {
            let mut answered = BTreeMap::new();
            let mut positions = BTreeMap::new();
            for mv in pos.legal_moves() {
                let uci = UciMove::from_move(mv, pos.castles().mode()).to_string();
                let mut next = pos.clone();
                next.play_unchecked(mv);
                let child_key = position_key(&next);
                answered.insert(
                    uci.clone(),
                    file.entries.iter().any(|entry| {
                        entry.side == side
                            && entry.key == child_key
                            && entry
                                .moves
                                .iter()
                                .any(|choice| legal_move(&next, &choice.uci).is_ok())
                    }),
                );
                positions.insert(
                    uci,
                    Fen::from_position(&next, EnPassantMode::Legal).to_string(),
                );
            }
            (answered, positions)
        };
        let entries: Vec<_> = file
            .entries
            .iter()
            .filter(|e| e.side == side)
            .filter_map(|e| {
                Self::view(
                    e,
                    practice
                        .positions
                        .get(&format!("{}|{}", side.label(), e.key))
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect();
        let current = entries.iter().find(|e| e.key == key).map(|e| PositionView {
            key: e.key.clone(),
            fen: e.fen.clone(),
            line: e.line.clone(),
            route: e.route.clone(),
            own_turn: e.own_turn,
            moves: e
                .moves
                .iter()
                .map(|m| ChoiceView {
                    uci: m.uci.clone(),
                    san: m.san.clone(),
                    preferred: m.preferred,
                    note: m.note.clone(),
                    valid: m.valid,
                })
                .collect(),
            practice: e.practice.clone(),
        });
        let legal = pos
            .legal_moves()
            .into_iter()
            .map(|m| {
                let uci = shakmaty::uci::UciMove::from_move(m, pos.castles().mode()).to_string();
                ChoiceView {
                    san: trees::san(&pos, &uci),
                    uci,
                    preferred: false,
                    note: String::new(),
                    valid: true,
                }
            })
            .collect();
        Ok(Snapshot {
            entries,
            current,
            legal,
            own_turn,
            answered_replies,
            reply_positions,
        })
    }
    pub fn edit(
        &self,
        line: Line,
        side: Side,
        uci: String,
        action: EditAction,
        note: Option<String>,
    ) -> Result<Snapshot, AppError> {
        let _guard = self.edit_lock.lock().map_err(io)?;
        let (pos, _, _) = trees::replay(&line)?;
        let key = position_key(&pos);
        if !matches!(action, EditAction::Remove) {
            legal_move(&pos, &uci)?;
        }
        if note.as_ref().is_some_and(|n| n.chars().count() > 280) {
            return Err(error("Notes are limited to 280 characters"));
        }
        let (mut file, _) = self.files()?;
        let idx = file
            .entries
            .iter()
            .position(|e| e.key == key && e.side == side);
        if idx.is_none() && !matches!(action, EditAction::Add) {
            return Err(error("Repertoire position not found"));
        }
        let idx = idx.unwrap_or_else(|| {
            file.entries.push(SavedPosition {
                key,
                side,
                line: line.clone(),
                moves: Vec::new(),
            });
            file.entries.len() - 1
        });
        let entry = &mut file.entries[idx];
        // A route is only a bookmark. A valid visit to the same canonical
        // position can repair an old or hand-edited route without changing
        // position identity or its saved choices.
        if trees::replay(&entry.line)
            .ok()
            .is_none_or(|(saved, _, _)| position_key(&saved) != entry.key)
        {
            entry.line = line.clone();
        }
        let move_idx = entry.moves.iter().position(|m| m.uci == uci);
        match action {
            EditAction::Add => {
                if move_idx.is_none() {
                    entry.moves.push(SavedMove {
                        uci,
                        preferred: entry.moves.is_empty(),
                        note: note.unwrap_or_default(),
                    });
                }
            }
            EditAction::Remove => {
                if let Some(i) = move_idx {
                    let was = entry.moves.remove(i).preferred;
                    if was && !entry.moves.is_empty() {
                        entry.moves[0].preferred = true;
                    }
                }
            }
            EditAction::Prefer => {
                let i = move_idx.ok_or_else(|| error("Move not saved"))?;
                for (j, m) in entry.moves.iter_mut().enumerate() {
                    m.preferred = j == i;
                }
            }
            EditAction::Note => {
                let i = move_idx.ok_or_else(|| error("Move not saved"))?;
                entry.moves[i].note = note.unwrap_or_default().trim().to_string();
            }
        }
        if entry.moves.is_empty() {
            file.entries.remove(idx);
        }
        file.version = 1;
        self.write("repertoire.json", &file)?;
        self.snapshot(line, side)
    }
    pub fn grade(&self, line: Line, side: Side, uci: String) -> Result<Grade, AppError> {
        let _guard = self.edit_lock.lock().map_err(io)?;
        let (pos, _, _) = trees::replay(&line)?;
        if !side.owns(&pos) {
            return Err(error("Choose a position where your side moves"));
        }
        legal_move(&pos, &uci)?;
        let key = position_key(&pos);
        let (file, mut practice) = self.files()?;
        let entry = file
            .entries
            .iter()
            .find(|e| e.key == key && e.side == side)
            .ok_or_else(|| error("No saved move at this position"))?;
        let valid: Vec<_> = entry
            .moves
            .iter()
            .filter(|m| legal_move(&pos, &m.uci).is_ok())
            .collect();
        if valid.is_empty() {
            return Err(error("No valid saved move at this position"));
        }
        let correct = valid.iter().any(|m| m.uci == uci);
        let progress = practice
            .positions
            .entry(format!("{}|{}", side.label(), key))
            .or_default();
        progress.attempts = progress.attempts.saturating_add(1);
        if correct {
            progress.correct = progress.correct.saturating_add(1);
            progress.needs_retry = false;
        } else {
            progress.needs_retry = true;
        }
        let progress = progress.clone();
        practice.version = 1;
        self.write("repertoire-practice.json", &practice)?;
        Ok(Grade {
            correct,
            played_san: trees::san(&pos, &uci),
            preferred_san: valid
                .iter()
                .find(|m| m.preferred)
                .or(valid.first())
                .map(|m| trees::san(&pos, &m.uci)),
            practice: progress,
        })
    }
}

#[tauri::command]
pub fn repertoire_snapshot(
    state: tauri::State<'_, RepertoireWorkspace>,
    line: Line,
    side: Side,
) -> Result<Snapshot, AppError> {
    state.snapshot(line, side)
}
#[tauri::command]
pub fn repertoire_edit(
    state: tauri::State<'_, RepertoireWorkspace>,
    line: Line,
    side: Side,
    uci: String,
    action: EditAction,
    note: Option<String>,
) -> Result<Snapshot, AppError> {
    state.edit(line, side, uci, action, note)
}
#[tauri::command]
pub fn repertoire_grade(
    state: tauri::State<'_, RepertoireWorkspace>,
    line: Line,
    side: Side,
    uci: String,
) -> Result<Grade, AppError> {
    state.grade(line, side, uci)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(moves: &[&str]) -> Line {
        Line {
            start_fen: chess_core::Game::initial().start_fen,
            moves: moves.iter().map(|m| (*m).into()).collect(),
        }
    }

    #[test]
    fn transposed_routes_share_white_choices_but_black_is_separate() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepertoireWorkspace::default();
        store.set_dir(dir.path().to_path_buf());
        let a = line(&["g1f3", "d7d5", "g2g3"]);
        let b = line(&["g2g3", "d7d5", "g1f3"]);
        store
            .edit(a.clone(), Side::White, "g8f6".into(), EditAction::Add, None)
            .unwrap();
        assert_eq!(
            store
                .snapshot(b.clone(), Side::White)
                .unwrap()
                .current
                .unwrap()
                .moves
                .len(),
            1
        );
        assert!(store.snapshot(b, Side::Black).unwrap().current.is_none());
    }

    #[test]
    fn edits_and_practice_survive_restart_in_separate_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = RepertoireWorkspace::default();
        a.set_dir(dir.path().to_path_buf());
        let start = line(&[]);
        a.edit(
            start.clone(),
            Side::White,
            "e2e4".into(),
            EditAction::Add,
            Some("Central space".into()),
        )
        .unwrap();
        assert!(
            !a.grade(start.clone(), Side::White, "d2d4".into())
                .unwrap()
                .correct
        );
        let b = RepertoireWorkspace::default();
        b.set_dir(dir.path().to_path_buf());
        let snapshot = b.snapshot(start.clone(), Side::White).unwrap();
        assert_eq!(snapshot.current.unwrap().moves[0].note, "Central space");
        assert!(snapshot.entries[0].practice.needs_retry);
        assert!(b.grade(start, Side::White, "e2e4".into()).unwrap().correct);
        assert!(
            !b.snapshot(line(&[]), Side::White).unwrap().entries[0]
                .practice
                .needs_retry
        );
        assert!(dir.path().join("repertoire.json").exists());
        assert!(dir.path().join("repertoire-practice.json").exists());
    }

    #[test]
    fn illegal_move_is_rejected_without_saving_and_saved_stale_move_is_visible() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepertoireWorkspace::default();
        store.set_dir(dir.path().to_path_buf());
        let start = line(&[]);
        assert!(
            store
                .edit(
                    start.clone(),
                    Side::White,
                    "e2e5".into(),
                    EditAction::Add,
                    None
                )
                .is_err()
        );
        assert!(
            store
                .snapshot(start.clone(), Side::White)
                .unwrap()
                .entries
                .is_empty()
        );
        std::fs::write(dir.path().join("repertoire.json"), r#"{"version":1,"entries":[{"key":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -","side":"white","line":{"startFen":"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQkq - 0 1","moves":[]},"moves":[{"uci":"e2e5","preferred":true,"note":"Old"}]}]}"#).unwrap();
        // A malformed bookmark falls back to the saved position and remains removable.
        let stale = store
            .snapshot(start.clone(), Side::White)
            .unwrap()
            .current
            .unwrap();
        assert_eq!(stale.route, "Saved position (original route unavailable)");
        assert!(!stale.moves[0].valid);
        let repaired = store
            .edit(start, Side::White, "e2e4".into(), EditAction::Add, None)
            .unwrap();
        let current = repaired.current.unwrap();
        assert_eq!(current.moves.len(), 2);
        assert!(!current.moves[0].valid);
        assert!(current.moves[1].valid);
    }

    #[test]
    fn black_practice_accepts_any_saved_choice_and_preference_is_only_feedback() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepertoireWorkspace::default();
        store.set_dir(dir.path().to_path_buf());
        let after_e4 = line(&["e2e4"]);
        store
            .edit(
                after_e4.clone(),
                Side::Black,
                "c7c5".into(),
                EditAction::Add,
                None,
            )
            .unwrap();
        store
            .edit(
                after_e4.clone(),
                Side::Black,
                "e7e5".into(),
                EditAction::Add,
                None,
            )
            .unwrap();
        let result = store
            .grade(after_e4.clone(), Side::Black, "e7e5".into())
            .unwrap();
        assert!(result.correct);
        assert_eq!(result.preferred_san.as_deref(), Some("c5"));
        assert!(store.grade(after_e4, Side::White, "d7d5".into()).is_err());
    }

    #[test]
    fn opponent_reply_is_answered_only_when_the_following_position_has_my_move() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepertoireWorkspace::default();
        store.set_dir(dir.path().to_path_buf());
        let after_e4 = line(&["e2e4"]);
        store
            .edit(
                after_e4.clone(),
                Side::White,
                "e7e5".into(),
                EditAction::Add,
                None,
            )
            .unwrap();
        let replies = store.snapshot(after_e4.clone(), Side::White).unwrap();
        assert_eq!(replies.answered_replies.get("e7e5"), Some(&false));
        store
            .edit(
                line(&["e2e4", "e7e5"]),
                Side::White,
                "g1f3".into(),
                EditAction::Add,
                None,
            )
            .unwrap();
        let replies = store.snapshot(after_e4, Side::White).unwrap();
        assert_eq!(replies.answered_replies.get("e7e5"), Some(&true));
        assert_eq!(replies.answered_replies.get("c7c5"), Some(&false));
    }

    #[test]
    fn route_after_null_move_remains_visible() {
        let dir = tempfile::tempdir().unwrap();
        let store = RepertoireWorkspace::default();
        store.set_dir(dir.path().to_path_buf());
        let after_null = line(&["0000"]);
        let saved = store
            .edit(
                after_null.clone(),
                Side::Black,
                "e7e5".into(),
                EditAction::Add,
                None,
            )
            .unwrap();
        assert_eq!(saved.current.unwrap().route, "--");
        assert_eq!(
            store
                .snapshot(after_null, Side::Black)
                .unwrap()
                .entries
                .len(),
            1
        );
    }
}
