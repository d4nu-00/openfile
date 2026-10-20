//! Parallel scans of a whole database. The entries are split into one
//! contiguous range per worker; each worker reads through its own handle
//! and keeps its first hits, so the joined result holds the first hits in
//! entry order.
use crate::{CbhError, Database};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Hits of a scan in entry order.
pub(crate) struct Scanned<T> {
    pub hits: Vec<(u64, T)>,
    /// Entries examined.
    pub scanned: u64,
    pub truncated: bool,
    pub cancelled: bool,
}

/// Workers for a scan: the machine's cores, at most 8.
pub fn default_workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
}

/// Scans entries `1..=count` of `db` with up to `workers` threads.
///
/// Each worker creates its state with `init`, then calls `chunk` for
/// consecutive runs of at most `step` entries `(start, n)`, which appends
/// its hits in entry order. A worker stops after `max_hits` hits of its own.
/// `progress` gets (entries done, count) about every 50 ms.
#[allow(clippy::too_many_arguments)]
pub(crate) fn parallel<S, T: Send>(
    db: &Database,
    workers: usize,
    step: u64,
    max_hits: usize,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
    init: impl Fn(&Database) -> Result<S, CbhError> + Sync,
    chunk: impl Fn(&Database, &mut S, u64, u64, &mut Vec<(u64, T)>) -> Result<(), CbhError> + Sync,
) -> Result<Scanned<T>, CbhError> {
    parallel_selected(
        db,
        None,
        workers,
        step,
        max_hits,
        cancel,
        &mut progress,
        init,
        chunk,
    )
}

/// Like `parallel`, but walks only a sorted unique selection, batching adjacent entries.
#[allow(clippy::too_many_arguments)]
pub(crate) fn parallel_selected<S, T: Send>(
    db: &Database,
    selection: Option<&[u64]>,
    workers: usize,
    step: u64,
    max_hits: usize,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
    init: impl Fn(&Database) -> Result<S, CbhError> + Sync,
    chunk: impl Fn(&Database, &mut S, u64, u64, &mut Vec<(u64, T)>) -> Result<(), CbhError> + Sync,
) -> Result<Scanned<T>, CbhError> {
    db.verify()?;
    let entries = db.info().entry_count;
    if selection.is_some_and(|s| {
        s.first() == Some(&0)
            || s.last().is_some_and(|&n| n > entries)
            || s.windows(2).any(|w| w[0] >= w[1])
    }) {
        return Err(CbhError::InvalidQuery(
            "Selection must contain sorted unique valid entries".into(),
        ));
    }
    let count = selection.map_or(entries, |s| s.len() as u64);
    let workers = (workers.clamp(1, 64) as u64).min(count.max(1));
    let handles = (1..workers)
        .map(|_| db.reopen())
        .collect::<Result<Vec<_>, _>>()?;
    let per = count.div_ceil(workers).max(1);
    let done = AtomicU64::new(0);
    // Hits found by each worker so far: a worker whose earlier ranges
    // already hold `max_hits` would only find hits that are dropped.
    let found: Vec<AtomicU64> = (0..workers).map(|_| AtomicU64::new(0)).collect();
    struct Part<T> {
        hits: Vec<(u64, T)>,
        scanned: u64,
        full: bool,
        cancelled: bool,
    }
    let parts = std::thread::scope(|scope| -> Result<Vec<Part<T>>, CbhError> {
        let mut running = vec![];
        for i in 0..workers {
            let first = 1 + i * per;
            if first > count {
                break;
            }
            let last = (first + per - 1).min(count);
            let handle = if i == 0 { db } else { &handles[i as usize - 1] };
            let (done, init, chunk, found) = (&done, &init, &chunk, &found);
            running.push(scope.spawn(move || -> Result<Part<T>, CbhError> {
                let mut state = init(handle)?;
                let mut part = Part {
                    hits: vec![],
                    scanned: 0,
                    full: false,
                    cancelled: false,
                };
                let mut at = first;
                while at <= last {
                    if cancel.load(Ordering::Relaxed) {
                        part.cancelled = true;
                        break;
                    }
                    let before: u64 = found[..i as usize]
                        .iter()
                        .map(|f| f.load(Ordering::Relaxed))
                        .sum();
                    if before >= max_hits as u64 {
                        part.full = true;
                        break;
                    }
                    let n = step.min(last + 1 - at);
                    if let Some(selection) = selection {
                        let picked = &selection[(at - 1) as usize..(at + n - 1) as usize];
                        let mut from = 0;
                        while from < picked.len() && !cancel.load(Ordering::Relaxed) {
                            let mut to = from + 1;
                            while to < picked.len() && picked[to] == picked[to - 1] + 1 {
                                to += 1;
                            }
                            chunk(
                                handle,
                                &mut state,
                                picked[from],
                                (to - from) as u64,
                                &mut part.hits,
                            )?;
                            from = to;
                        }
                    } else {
                        chunk(handle, &mut state, at, n, &mut part.hits)?;
                    }
                    found[i as usize].store(part.hits.len() as u64, Ordering::Relaxed);
                    part.scanned += n;
                    done.fetch_add(n, Ordering::Relaxed);
                    at += n;
                    if part.hits.len() >= max_hits {
                        // Entries left unread may hold more hits.
                        part.full = at <= last;
                        break;
                    }
                }
                Ok(part)
            }));
        }
        loop {
            let finished = running.iter().all(|p| p.is_finished());
            progress(done.load(Ordering::Relaxed), count);
            if finished {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        running
            .into_iter()
            .map(|p| {
                p.join()
                    .map_err(|_| CbhError::Io(std::io::Error::other("scan worker stopped")))?
            })
            .collect()
    })?;
    let mut out = Scanned {
        hits: vec![],
        scanned: 0,
        truncated: false,
        cancelled: false,
    };
    for part in parts {
        out.scanned += part.scanned;
        out.cancelled |= part.cancelled;
        out.truncated |= part.full;
        for hit in part.hits {
            if out.hits.len() == max_hits {
                out.truncated = true;
                break;
            }
            out.hits.push(hit);
        }
    }
    out.cancelled |= cancel.load(Ordering::Relaxed);
    Ok(out)
}
