use cbh_reader::{
    Database, ReadLimits,
    v2::{
        recover,
        write::{WRITTEN, Writer},
    },
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

fn fixture(dir: &Path) -> PathBuf {
    let path = dir.join("Source.2cbh");
    let mut writer = Writer::create(&path).unwrap();
    for (i, moves) in [
        "1. e4 {Centre} e5 (1... c5) 2. Nf3",
        "1. d4 d5",
        "1. c4 e5",
        "1. Nf3 d5",
    ]
    .iter()
    .enumerate()
    {
        let text = format!(
            "[White \"Player {i}\"]\n[Black \"Other\"]\n[Event \"Study\"]\n[Site \"London\"]\n[Date \"2026.10.22\"]\n[Result \"1-0\"]\n\n{moves} 1-0"
        );
        let game = chess_core::parse_pgn(text.as_bytes(), &Default::default())
            .unwrap()
            .remove(0);
        writer.append(&game).unwrap();
    }
    writer.set_deleted(4, true).unwrap();
    drop(writer);
    path
}
fn change(path: &Path, edit: impl FnOnce(&mut Vec<u8>)) {
    let mut b = std::fs::read(path).unwrap();
    edit(&mut b);
    std::fs::write(path, b).unwrap();
}
fn offset(path: &Path, entry: usize, field: usize) -> usize {
    let b = std::fs::read(path).unwrap();
    u64::from_le_bytes(
        b[entry * 192 + field..entry * 192 + field + 8]
            .try_into()
            .unwrap(),
    ) as usize
}
fn run(path: &Path, dest: &Path) -> recover::RecoveryReport {
    recover::recover(path, dest, &AtomicBool::new(false), |_, _, _| {}).unwrap()
}
fn clean(path: &Path, count: u64) {
    let db = Database::open(path, ReadLimits::default()).unwrap();
    assert_eq!(db.info().entry_count, count);
    let checked = db.check_integrity(&AtomicBool::new(false), |_| {}).unwrap();
    assert!(checked.problems.is_empty(), "{:?}", checked.problems);
}

#[test]
fn damaged_lower_count_does_not_silently_hide_complete_records() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    change(&source, |b| b[16..24].copy_from_slice(&2u64.to_le_bytes()));
    let r = run(&source, &d.path().join("Recovered"));
    assert_eq!((r.checked, r.recovered), (4, 4));
    assert!(r.problems.iter().any(|p| p.code == "headerCount"));
    let log = std::fs::read_to_string(r.report_path.unwrap()).unwrap();
    assert!(log.contains("Outside the declared count"));
    clean(&r.path.unwrap(), 4);
}

#[test]
fn damaged_counts_and_stream_headers_recover_losslessly_without_opening_source() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let mut original = Database::open(&source, ReadLimits::default()).unwrap();
    let games: Vec<_> = (1..=4)
        .map(|n| original.load(n).unwrap().game.unwrap())
        .collect();
    drop(original);
    change(&source, |b| {
        b[16..24].copy_from_slice(&999u64.to_le_bytes())
    });
    change(&source.with_extension("2cbg"), |b| b[..12].fill(0));
    let before: Vec<_> = WRITTEN
        .iter()
        .map(|e| std::fs::read(source.with_extension(e)).unwrap())
        .collect();
    assert!(Database::open(&source, ReadLimits::default()).is_err());
    let report = run(&source, &d.path().join("Recovered"));
    assert_eq!(
        (report.recovered, report.corrupt, report.unsupported),
        (4, 0, 0)
    );
    let output = report.path.unwrap();
    clean(&output, 4);
    let mut db = Database::open(&output, ReadLimits::default()).unwrap();
    for (i, expected) in games.iter().enumerate() {
        assert_eq!(&db.load(i as u64 + 1).unwrap().game.unwrap(), expected);
    }
    assert!(db.load(4).unwrap().summary.deleted);
    for (i, e) in WRITTEN.iter().enumerate() {
        assert_eq!(std::fs::read(source.with_extension(e)).unwrap(), before[i]);
    }
    assert!(report.problems.iter().any(|p| p.code == "headerCount"));
}

#[test]
fn independently_damaged_blocks_references_and_truncated_records_are_itemised() {
    for damage in [
        "moves",
        "annotations",
        "name",
        "truncatedMoves",
        "truncatedIndex",
        "missingAnnotations",
        "truncatedNames",
    ] {
        let d = tempfile::tempdir().unwrap();
        let source = fixture(d.path());
        match damage {
            "moves" => {
                let at = offset(&source, 2, 8);
                change(&source.with_extension("2cbg"), |b| b[at + 26] ^= 1);
            }
            "annotations" => {
                let at = offset(&source, 2, 16);
                change(&source.with_extension("2cba"), |b| b[at + 26] ^= 1);
            }
            "name" => change(&source, |b| {
                b[2 * 192 + 24..2 * 192 + 32].copy_from_slice(&999u64.to_le_bytes())
            }),
            "truncatedMoves" => {
                let at = offset(&source, 4, 8);
                change(&source.with_extension("2cbg"), |b| b.truncate(at + 27));
            }
            "truncatedIndex" => change(&source, |b| b.truncate(4 * 192 + 30)),
            "missingAnnotations" => std::fs::remove_file(source.with_extension("2cba")).unwrap(),
            "truncatedNames" => change(&source.with_extension("2lid"), |b| {
                b.truncate(184 + 2 * 4234)
            }),
            _ => unreachable!(),
        }
        let report = run(&source, &d.path().join("Recovered"));
        let n = match damage {
            "missingAnnotations" => 0,
            "truncatedNames" => 1,
            _ => 3,
        };
        assert_eq!(report.recovered, n, "{damage}: {report:?}");
        assert!(report.corrupt > 0, "{damage}");
        if let Some(output) = report.path {
            clean(&output, n);
        }
        let log = std::fs::read_to_string(report.report_path.unwrap()).unwrap();
        assert!(log.contains("corrupt"), "{damage}");
        assert!(
            log.lines()
                .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
        );
    }
}

#[test]
fn cancellation_mutation_and_destination_collision_never_publish_a_partial_family() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let dest = d.path().join("Recovered");
    let stop = AtomicBool::new(false);
    let r = recover::recover(&source, &dest, &stop, |phase, done, _| {
        if phase == "recovering" && done >= 1 {
            stop.store(true, Ordering::Relaxed)
        }
    })
    .unwrap();
    assert!(r.cancelled);
    assert!(!dest.exists());
    assert!(recover::recover(&source, d.path(), &AtomicBool::new(false), |_, _, _| {}).is_err());
    let changed = AtomicBool::new(false);
    let result = recover::recover(&source, &dest, &AtomicBool::new(false), |phase, done, _| {
        if phase == "recovering" && done >= 1 && !changed.swap(true, Ordering::Relaxed) {
            change(&source, |b| b[192 + 130] ^= 1);
        }
    });
    assert!(matches!(result, Err(cbh_reader::CbhError::SourceChanged)));
    assert!(!dest.exists());
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(dest.join("keep"), "mine").unwrap();
    assert!(recover::recover(&source, &dest, &AtomicBool::new(false), |_, _, _| {}).is_err());
    assert_eq!(std::fs::read_to_string(dest.join("keep")).unwrap(), "mine");
}

#[test]
fn collision_appearing_during_verification_is_preserved() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let dest = d.path().join("Recovered");
    let result = recover::recover(&source, &dest, &AtomicBool::new(false), |phase, _, _| {
        if phase == "verifying" && !dest.exists() {
            std::fs::create_dir(&dest).unwrap();
        }
    });
    assert!(result.is_err());
    assert!(dest.is_dir());
    assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 5);
}

#[test]
fn special_results_and_opaque_metadata_are_retained_and_unknown_kinds_are_reported() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    change(&source, |b| {
        for (i, code) in [4, 5, 6, 7].iter().enumerate() {
            b[(i + 1) * 192 + 88] = *code;
            b[(i + 1) * 192 + 148] = 42;
        }
        b[3 * 192 + 2..3 * 192 + 4].copy_from_slice(&0x9901u16.to_le_bytes());
    });
    let r = run(&source, &d.path().join("Recovered"));
    assert_eq!((r.recovered, r.unsupported), (3, 1));
    let out = r.path.unwrap();
    clean(&out, 3);
    let b = std::fs::read(out).unwrap();
    assert_eq!([b[192 + 88], b[384 + 88], b[576 + 88]], [4, 5, 7]);
    assert_eq!([b[192 + 148], b[384 + 148], b[576 + 148]], [42, 42, 42]);
}

#[test]
fn opaque_annotations_are_retained_verbatim_and_reported_as_not_displayable() {
    use cbh_reader::v2::block::checksum;
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let at = offset(&source, 2, 16);
    let payload = [0, 0, 0, 0, 1, 0, 0, 0, 0x77, 0x77, 42, 99];
    change(&source.with_extension("2cba"), |b| {
        let len = u32::from_le_bytes(b[at + 8..at + 12].try_into().unwrap()) as usize;
        let padding = u32::from_le_bytes(b[at + 12..at + 16].try_into().unwrap()) as usize;
        b[at + 8..at + 12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        b[at + 12..at + 16]
            .copy_from_slice(&((len + padding - payload.len()) as u32).to_le_bytes());
        b[at + 16..at + 24].copy_from_slice(&checksum(&payload).to_le_bytes());
        b[at + 26..at + 26 + payload.len()].copy_from_slice(&payload);
    });
    let r = run(&source, &d.path().join("Recovered"));
    assert_eq!((r.recovered, r.corrupt), (4, 0));
    assert_eq!(r.warnings, 1);
    let out = r.path.unwrap();
    clean(&out, 4);
    let recovered_at = offset(&out, 2, 16);
    let b = std::fs::read(out.with_extension("2cba")).unwrap();
    assert_eq!(
        &b[recovered_at + 26..recovered_at + 26 + payload.len()],
        &payload
    );
    assert!(
        std::fs::read_to_string(r.report_path.unwrap())
            .unwrap()
            .contains("cannot display")
    );
}

#[test]
fn annotator_source_and_team_references_remap_without_losing_slot_tails() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let field = |s: &str| {
        let mut b = (s.len() as u32).to_le_bytes().to_vec();
        b.extend(s.as_bytes());
        b
    };
    let mut source_payload = field("Bulletin");
    source_payload.extend(field("Publisher"));
    source_payload.extend([0u8; 12]);
    let mut team_payload = field("Study team");
    team_payload.extend([12, 34, 56, 78, 90]);
    let mut slots = vec![];
    change(&source.with_extension("2lid"), |b| {
        for (kind, id, relative, size, payload) in [
            (2usize, 2usize, 2144usize, 220usize, source_payload),
            (4, 3, 3388, 314, team_payload),
        ] {
            b[16 + kind * 20..20 + kind * 20].copy_from_slice(&((id + 1) as u32).to_be_bytes());
            let mut slot = vec![0xa5; size];
            slot[..4].copy_from_slice(&(payload.len() as u32).to_le_bytes());
            slot[4..4 + payload.len()].copy_from_slice(&payload);
            let at = 184 + id * 4234 + relative;
            b[at..at + size].copy_from_slice(&slot);
            slots.push((relative, slot));
        }
    });
    change(&source, |b| {
        for (at, id) in [(48, 4u64), (56, 2), (64, 3)] {
            b[192 + at..192 + at + 8].copy_from_slice(&id.to_le_bytes());
        }
    });
    let r = run(&source, &d.path().join("Recovered"));
    let out = r.path.unwrap();
    clean(&out, 4);
    let b = std::fs::read(out.with_extension("2lid")).unwrap();
    for (relative, slot) in slots {
        assert_eq!(&b[184 + relative..184 + relative + slot.len()], &slot);
    }
    let mut db = Database::open(&out, ReadLimits::default()).unwrap();
    let e = db.load(1).unwrap();
    assert_eq!(e.summary.annotator.as_deref(), Some("Player 3"));
    assert_eq!(e.summary.source.as_deref(), Some("Bulletin"));
    assert_eq!(e.summary.source_publisher.as_deref(), Some("Publisher"));
    assert_eq!(e.summary.white_team.as_deref(), Some("Study team"));
}

#[test]
fn verification_cancellation_and_new_source_companions_prevent_publication() {
    for mutation in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let source = fixture(d.path());
        let dest = d.path().join("Recovered");
        if mutation {
            std::fs::remove_file(source.with_extension("2cba")).unwrap();
            change(&source, |b| {
                for n in 1..=4 {
                    b[n * 192 + 16..n * 192 + 24].fill(0);
                }
            });
        }
        let cancel = AtomicBool::new(false);
        let once = AtomicBool::new(false);
        let r = recover::recover(&source, &dest, &cancel, |phase, _, _| {
            if phase == "verifying" && !once.swap(true, Ordering::Relaxed) {
                if mutation {
                    std::fs::write(source.with_extension("2cba"), [0u8; 12]).unwrap();
                } else {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        });
        if mutation {
            assert!(matches!(r, Err(cbh_reader::CbhError::SourceChanged)));
        } else {
            assert!(r.unwrap().cancelled);
        }
        assert!(!dest.exists());
    }
}

#[test]
fn interrupted_worker() {
    if let Some(root) = std::env::var_os("OPENFILE_RECOVERY_CRASH_FIXTURE") {
        let root = PathBuf::from(root);
        let _ = recover::recover(
            &root.join("Source.2cbh"),
            &root.join("Recovered"),
            &AtomicBool::new(false),
            |phase, _, _| {
                if phase == "verifying" {
                    std::process::exit(73);
                }
            },
        );
        panic!("worker did not reach the interruption point");
    }
}

#[test]
fn process_interruption_leaves_no_published_partial_family_and_retry_preserves_leftovers() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let before = std::fs::read(&source).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "interrupted_worker"])
        .env("OPENFILE_RECOVERY_CRASH_FIXTURE", d.path())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    assert!(!d.path().join("Recovered").exists());
    let leftovers: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".openfile-recovery-")
                .then_some(p)
        })
        .collect();
    assert_eq!(leftovers.len(), 1);
    let r = run(&source, &d.path().join("Recovered"));
    clean(&r.path.unwrap(), 4);
    assert!(leftovers[0].is_dir());
    assert_eq!(std::fs::read(source).unwrap(), before);
}

#[test]
fn diagnostics_are_bounded_but_every_unsupported_entry_is_logged() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    change(&source, |b| {
        let mut text = b[192..384].to_vec();
        text[2..4].copy_from_slice(&0x0301u16.to_le_bytes());
        b.truncate(192);
        for _ in 0..600 {
            b.extend_from_slice(&text);
        }
        b[16..24].copy_from_slice(&601u64.to_le_bytes());
    });
    let r = run(&source, &d.path().join("Recovered"));
    assert_eq!((r.recovered, r.unsupported), (0, 600));
    assert_eq!(r.problems.len(), 500);
    assert!(r.problems_truncated);
    let log = std::fs::read_to_string(r.report_path.unwrap()).unwrap();
    assert_eq!(
        log.lines()
            .filter(|line| line.contains("unsupported"))
            .count(),
        600
    );
    clean(&r.path.unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn failed_write_worker() {
    if let Some(root) = std::env::var_os("OPENFILE_RECOVERY_WRITE_FAILURE") {
        let root = PathBuf::from(root);
        let cap = libc::rlimit {
            rlim_cur: 1024,
            rlim_max: 1024,
        };
        // SAFETY: only this isolated test child changes its own file-size limit.
        unsafe {
            libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &cap), 0);
        }
        let result = recover::recover(
            &root.join("Source.2cbh"),
            &root.join("Recovered"),
            &AtomicBool::new(false),
            |_, _, _| {},
        );
        assert!(
            matches!(result, Err(cbh_reader::CbhError::Io(_))),
            "{result:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn actual_file_size_write_failure_cleans_temporary_output_and_preserves_source() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let before: Vec<_> = WRITTEN
        .iter()
        .map(|ext| std::fs::read(source.with_extension(ext)).unwrap())
        .collect();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "failed_write_worker"])
        .env("OPENFILE_RECOVERY_WRITE_FAILURE", d.path())
        // Pipes keep the child's file-size cap from also limiting a redirected
        // parent test log, which may already be larger than the imposed cap.
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{:?}: {} {}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!d.path().join("Recovered").exists());
    assert!(!std::fs::read_dir(d.path()).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".openfile-recovery-")
    }));
    for (ext, bytes) in WRITTEN.iter().zip(before) {
        assert_eq!(std::fs::read(source.with_extension(ext)).unwrap(), bytes);
    }
}
