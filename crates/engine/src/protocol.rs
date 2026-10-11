#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Score {
    Cp { value: i32, bound: Option<String> },
    Mate { moves: i32, bound: Option<String> },
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AnalysisInfo {
    pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub multipv: u32,
    pub score: Option<Score>,
    pub pv: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UciResponse {
    Id {
        field: String,
        value: String,
    },
    Option(crate::EngineOption),
    UciOk,
    ReadyOk,
    Info(AnalysisInfo),
    BestMove {
        best: String,
        ponder: Option<String>,
    },
}

use crate::EngineError;
pub const MAX_LINE: usize = 16 * 1024;
fn malformed(field: &str) -> EngineError {
    EngineError::Protocol(format!("Invalid {field}"))
}
fn number<T: std::str::FromStr>(
    tokens: &[&str],
    i: &mut usize,
    field: &str,
) -> Result<T, EngineError> {
    let token = tokens.get(*i).ok_or_else(|| malformed(field))?;
    *i += 1;
    token.parse().map_err(|_| malformed(field))
}
pub(crate) fn is_move(s: &str) -> bool {
    if s == "0000" {
        return true;
    }
    let b = s.as_bytes();
    matches!(b.len(), 4 | 5)
        && (b'a'..=b'h').contains(&b[0])
        && (b'1'..=b'8').contains(&b[1])
        && (b'a'..=b'h').contains(&b[2])
        && (b'1'..=b'8').contains(&b[3])
        && (b.len() == 4 || matches!(b[4], b'q' | b'r' | b'b' | b'n'))
}
pub fn parse_line(line: &str) -> Result<Option<UciResponse>, EngineError> {
    if line.len() > MAX_LINE {
        return Err(EngineError::LineLimit);
    }
    if line.contains(['\r', '\n']) {
        return Err(malformed("line boundary"));
    }
    let t: Vec<_> = line.split_whitespace().collect();
    let Some(&first) = t.first() else {
        return Ok(None);
    };
    Ok(Some(match first {
        "uciok" => UciResponse::UciOk,
        "readyok" => UciResponse::ReadyOk,
        "id" => {
            let field = t.get(1).ok_or_else(|| malformed("id"))?;
            UciResponse::Id {
                field: (*field).into(),
                value: t.get(2..).unwrap_or(&[]).join(" "),
            }
        }
        "option" => UciResponse::Option(crate::options::parse_option(line)?),
        "bestmove" => {
            let best = *t.get(1).ok_or_else(|| malformed("bestmove"))?;
            if !is_move(best) && best != "(none)" {
                return Err(malformed("bestmove"));
            }
            let ponder = if t.get(2) == Some(&"ponder") {
                let value = *t.get(3).ok_or_else(|| malformed("ponder"))?;
                if !is_move(value) {
                    return Err(malformed("ponder"));
                }
                Some(value.into())
            } else {
                None
            };
            UciResponse::BestMove {
                best: best.into(),
                ponder,
            }
        }
        "info" => {
            let mut info = AnalysisInfo {
                depth: None,
                nodes: None,
                multipv: 1,
                score: None,
                pv: vec![],
            };
            let mut i = 1;
            while i < t.len() {
                let field = t[i];
                i += 1;
                match field {
                    "depth" => info.depth = Some(number(&t, &mut i, "depth")?),
                    "nodes" => info.nodes = Some(number(&t, &mut i, "nodes")?),
                    "multipv" => {
                        info.multipv = number(&t, &mut i, "multipv")?;
                        if info.multipv == 0 {
                            return Err(malformed("multipv"));
                        }
                    }
                    "score" => {
                        let kind = *t.get(i).ok_or_else(|| malformed("score"))?;
                        i += 1;
                        let value = number::<i32>(&t, &mut i, "score")?;
                        let bound = if t
                            .get(i)
                            .is_some_and(|s| matches!(*s, "lowerbound" | "upperbound"))
                        {
                            let v = t[i].to_string();
                            i += 1;
                            Some(v)
                        } else {
                            None
                        };
                        info.score = Some(match kind {
                            "cp" => Score::Cp { value, bound },
                            "mate" => Score::Mate {
                                moves: value,
                                bound,
                            },
                            _ => return Err(malformed("score kind")),
                        });
                    }
                    "pv" => {
                        let begin = i;
                        while i < t.len() && is_move(t[i]) {
                            info.pv.push(t[i].into());
                            i += 1;
                        }
                        if i == begin {
                            return Err(malformed("pv"));
                        }
                    }
                    "string" => break,
                    _ => {} // Unknown tokens cannot swallow following known fields.
                }
            }
            UciResponse::Info(info)
        }
        _ => return Ok(None),
    }))
}
pub fn position_command(fen: &str, history: &[String]) -> Result<String, EngineError> {
    if fen.contains(['\r', '\n']) || history.iter().any(|v| v.contains(['\r', '\n'])) {
        return Err(malformed("position boundary"));
    }
    let game = chess_core::Game::from_fen(fen).map_err(|e| EngineError::Position(e.to_string()))?;
    let mut position = chess_core::model::parse_position(&game.start_fen)
        .map_err(|e| EngineError::Position(e.to_string()))?;
    for uci in history {
        position = chess_core::model::apply_uci(position, uci)
            .map_err(|e| EngineError::Position(e.to_string()))?;
    }
    let mut command = format!("position fen {}", game.start_fen);
    if !history.is_empty() {
        command.push_str(" moves ");
        command.push_str(&history.join(" "));
    }
    if command.len() > MAX_LINE {
        return Err(EngineError::LineLimit);
    }
    Ok(command)
}
