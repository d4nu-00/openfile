//! Running the builder over a database list and writing the training
//! databases (the database window's Build Mate Training).
use crate::{
    clipboard::RowRange,
    commands::AppError,
    convert::{check_target, partial, remove_family},
    mate_training::{
        build::{BuildOptions, Collected, ordered},
        verify::{CombinedVerifier, EngineVerifier, MateVerifier},
    },
    registry::Registry,
};
use cbh_reader::v2::write::{WRITTEN, Writer};
use chess_core::Game;
use engine::EngineSpec;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Source games used at most.
pub const MAX_GAMES: usize = 20_000_000;

/// Stop and Cancel of a running build.
#[derive(Default)]
pub struct Control {
    /// Stop looking and keep what was found.
    pub stop: AtomicBool,
    /// Stop looking and write nothing.
    pub cancel: AtomicBool,
}

/// Puzzles of a combination at each depth, and its chains.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinationCount {
    pub combination: String,
    /// Index 0 is mate in 1.
    pub depths: [u32; 5],
    pub chains: u32,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MateTrainingProgress {
    pub scanned: String,
    pub total: String,
    /// Puzzles at each depth; index 0 is mate in 1.
    pub puzzles: [u32; 5],
    pub chains: u32,
    pub combinations: Vec<CombinationCount>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenDatabase {
    pub path: String,
    /// The depth of a mate-in-N database; `None` for the chains.
    pub depth: Option<u8>,
    pub games: u32,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    pub kind: String,
    pub count: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MateTrainingResponse {
    pub databases: Vec<WrittenDatabase>,
    pub combinations: Vec<CombinationCount>,
    pub puzzles: [u32; 5],
    pub chains: u32,
    pub rejections: Vec<Rejection>,
    pub scanned: String,
    pub total: String,
    /// Games of a combination that was not asked for.
    pub filtered_out: String,
    /// Stopped by the user; what was found is written.
    pub stopped: bool,
    /// Cancelled by the user; nothing is written.
    pub cancelled: bool,
    /// Every quota was met before the games ran out.
    pub complete: bool,
}

impl MateTrainingResponse {
    /// The response for a finished collection of `total` source games.
    pub fn new(
        collected: &Collected,
        outcome: Outcome,
        total: u64,
        databases: Vec<WrittenDatabase>,
    ) -> Self {
        Self {
            databases,
            combinations: counts(collected),
            puzzles: std::array::from_fn(|i| collected.puzzles(i as u8 + 1).len() as u32),
            chains: collected.chains().len() as u32,
            rejections: collected
                .rejections
                .iter()
                .map(|(kind, count)| Rejection {
                    kind: kind.clone(),
                    count: count.to_string(),
                })
                .collect(),
            scanned: collected.scanned.to_string(),
            total: total.to_string(),
            filtered_out: collected.filtered_out.to_string(),
            stopped: outcome.stopped,
            cancelled: outcome.cancelled,
            complete: outcome.complete,
        }
    }
}

pub struct MateTrainingParams {
    pub database_id: String,
    pub ranges: Vec<RowRange>,
    pub options: BuildOptions,
    /// The engine for mates in 3 to 5.
    pub engine: Option<PathBuf>,
    /// The engine's time for each check.
    pub millis: u64,
    pub folder: PathBuf,
    pub name: String,
}

/// What ended a collection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub stopped: bool,
    pub cancelled: bool,
    /// Every quota was met before the games ran out.
    pub complete: bool,
}

/// Looks at the games `ordinals` of `source` in order until they run out,
/// every quota is met, or `control` says to stop. `progress` gets the games
/// looked at, the total and what has been found, after each game.
pub fn collect(
    source: &mut cbh_reader::Database,
    ordinals: &[u64],
    options: &BuildOptions,
    verifier: &mut dyn MateVerifier,
    control: &Control,
    mut progress: impl FnMut(u64, u64, &Collected),
) -> Result<(Collected, Outcome), AppError> {
    let mut collected = Collected::new(options)?;
    let mut outcome = Outcome::default();
    let total = ordinals.len() as u64;
    progress(0, total, &collected);
    for &ordinal in ordinals {
        if control.cancel.load(Ordering::Relaxed) {
            outcome.cancelled = true;
            break;
        }
        if control.stop.load(Ordering::Relaxed) {
            outcome.stopped = true;
            break;
        }
        if collected.full() {
            break;
        }
        match source.load(ordinal) {
            Ok(loaded) => match loaded.game {
                Some(game) => collected.add_game(ordinal, &game, verifier)?,
                None => collected.skip_unreadable(),
            },
            Err(cbh_reader::CbhError::SourceChanged) => {
                return Err(cbh_reader::CbhError::SourceChanged.into());
            }
            Err(_) => collected.skip_unreadable(),
        }
        progress(collected.scanned, total, &collected);
    }
    outcome.complete = !outcome.stopped && !outcome.cancelled && collected.full();
    Ok((collected, outcome))
}

/// The file of a depth database (`depth`) or the chain database (`None`).
fn target(folder: &Path, name: &str, depth: Option<u8>) -> PathBuf {
    folder.join(match depth {
        Some(n) => format!("{name} - Mate in {n}.2cbh"),
        None => format!("{name} - Mate chains.2cbh"),
    })
}

/// The name of the databases: one file name, not a path.
fn clean_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', '\0']) {
        return Err(AppError::new(
            "invalidPath",
            "Choose a name for the databases that is not a path",
        ));
    }
    Ok(name.to_string())
}

/// Writes the depth databases and the chain database that have games, into
/// `folder`. The games go to temporary databases first, which are renamed
/// when all are written; a failure leaves nothing. Existing databases are
/// never replaced.
pub fn write_databases(
    collected: &Collected,
    options: &BuildOptions,
    folder: &Path,
    name: &str,
    sources: &[PathBuf],
) -> Result<Vec<WrittenDatabase>, AppError> {
    let (depths, chains) = ordered(collected);
    let mut jobs: Vec<(Option<u8>, PathBuf, Vec<Game>)> = vec![];
    for depth in options.min_depth..=options.max_depth {
        let games = depths[usize::from(depth) - 1].clone();
        if !games.is_empty() {
            jobs.push((Some(depth), target(folder, name, Some(depth)), games));
        }
    }
    if !chains.is_empty() {
        jobs.push((None, target(folder, name, None), chains));
    }
    for (_, path, _) in &jobs {
        check_target(path.clone(), sources)?;
    }
    // The files moved into place, to remove again when a later one fails.
    let mut renamed: Vec<PathBuf> = vec![];
    let written = (|| -> Result<(), AppError> {
        for (_, path, games) in &jobs {
            let temp = partial(path);
            remove_family(&temp);
            let mut writer = Writer::create(&temp)?;
            writer.append_all(games)?;
            drop(writer);
        }
        // A target may have appeared meanwhile; never replace it.
        for (_, path, _) in &jobs {
            check_target(path.clone(), &[])?;
        }
        for (_, path, _) in &jobs {
            for ext in WRITTEN {
                let to = path.with_extension(ext);
                std::fs::rename(partial(path).with_extension(ext), &to)
                    .map_err(|e| AppError::new("io", e.to_string()))?;
                renamed.push(to);
            }
        }
        Ok(())
    })();
    if let Err(e) = written {
        for (_, path, _) in &jobs {
            remove_family(&partial(path));
        }
        for file in renamed {
            let _ = std::fs::remove_file(file);
        }
        return Err(e);
    }
    Ok(jobs
        .into_iter()
        .map(|(depth, path, games)| WrittenDatabase {
            path: path.to_string_lossy().into_owned(),
            depth,
            games: games.len() as u32,
        })
        .collect())
}

fn counts(collected: &Collected) -> Vec<CombinationCount> {
    collected
        .combinations()
        .into_iter()
        .map(|combination| CombinationCount {
            depths: std::array::from_fn(|i| collected.count(&combination, i as u8 + 1)),
            chains: collected.chain_count(&combination),
            combination,
        })
        .collect()
}

impl MateTrainingProgress {
    pub fn of(scanned: u64, total: u64, collected: &Collected) -> Self {
        Self {
            scanned: scanned.to_string(),
            total: total.to_string(),
            puzzles: std::array::from_fn(|i| collected.puzzles(i as u8 + 1).len() as u32),
            chains: collected.chains().len() as u32,
            combinations: counts(collected),
        }
    }
}

impl Registry {
    /// Build Mate Training: collects puzzles from the games in `params.ranges`
    /// of the current list and writes the databases into `params.folder`.
    pub fn build_mate_training(
        &self,
        params: MateTrainingParams,
        progress: impl FnMut(u64, u64, &Collected),
    ) -> Result<MateTrainingResponse, AppError> {
        let MateTrainingParams {
            database_id,
            ranges,
            options,
            engine,
            millis,
            folder,
            name,
        } = params;
        options.check()?;
        if !(100..=60_000).contains(&millis) {
            return Err(AppError::new(
                "invalidParameters",
                "The time per check must be from 0.1 to 60 seconds",
            ));
        }
        let name = clean_name(&name)?;
        if !folder.is_dir() {
            return Err(AppError::new("invalidPath", "Choose an existing folder"));
        }
        let engine = match (options.max_depth > 2, engine) {
            (true, None) => {
                return Err(AppError::new(
                    "engine",
                    "Choose an engine for mates in 3 to 5",
                ));
            }
            (true, Some(path)) => Some(path),
            (false, _) => None,
        };
        let (ordinals, truncated) = self.ordinals_of_rows(&database_id, &ranges, MAX_GAMES)?;
        if truncated {
            return Err(AppError::new(
                "limitExceeded",
                format!("Use at most {MAX_GAMES} games"),
            ));
        }
        let source_path = self
            .lock()?
            .paths
            .get(&database_id)
            .cloned()
            .ok_or_else(|| AppError::new("invalidHandle", "Database is closed or unknown"))?;
        // A separate handle keeps the list usable while the build runs.
        let mut source =
            cbh_reader::Database::open(&source_path, cbh_reader::ReadLimits::default())?;
        // Refuse before the long part when a database could not be written.
        let sources = source.source_paths().to_vec();
        for depth in options.min_depth..=options.max_depth {
            check_target(target(&folder, &name, Some(depth)), &sources)?;
        }
        check_target(target(&folder, &name, None), &sources)?;
        let mut verifier = match engine {
            Some(executable) => CombinedVerifier::new(EngineVerifier::spawn(
                EngineSpec {
                    executable,
                    arguments: vec![],
                },
                millis,
            )?),
            None => CombinedVerifier::solver_only(),
        };
        let control = Arc::new(Control::default());
        if let Some(old) = self
            .lock()?
            .mate_training
            .insert(database_id.clone(), control.clone())
        {
            old.cancel.store(true, Ordering::Relaxed);
        }
        let result = collect(
            &mut source,
            &ordinals,
            &options,
            &mut verifier,
            &control,
            progress,
        );
        if let Ok(mut state) = self.lock()
            && state
                .mate_training
                .get(&database_id)
                .is_some_and(|c| Arc::ptr_eq(c, &control))
        {
            state.mate_training.remove(&database_id);
        }
        drop(verifier);
        let (collected, outcome) = result?;
        let databases = if outcome.cancelled {
            vec![]
        } else {
            write_databases(&collected, &options, &folder, &name, &sources)?
        };
        Ok(MateTrainingResponse::new(
            &collected,
            outcome,
            ordinals.len() as u64,
            databases,
        ))
    }

    /// Stop: the build keeps what it found.
    pub fn stop_mate_training(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.mate_training.get(&database_id) {
            c.stop.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Cancel: the build writes nothing.
    pub fn cancel_mate_training(&self, database_id: String) -> Result<(), AppError> {
        if let Some(c) = self.lock()?.mate_training.get(&database_id) {
            c.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}
