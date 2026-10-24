use cbh_reader::v2::write::Writer;
use openfile::recovery::RecoveryJobs;

#[test]
fn cancelled_pending_and_stale_jobs_do_not_publish_or_cancel_another_job() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("Broken.2cbh");
    drop(Writer::create(&source).unwrap());
    let jobs = RecoveryJobs::default();
    let old = jobs.begin().unwrap();
    assert!(jobs.begin().is_err());
    jobs.cancel(&old).unwrap();
    let r = jobs
        .run(&old, &source, &d.path().join("Cancelled"), |_| {})
        .unwrap_err();
    assert_eq!(r.code, "staleJob");
    assert!(!d.path().join("Cancelled").exists());
    let current = jobs.begin().unwrap();
    jobs.cancel(&old).unwrap();
    assert!(
        jobs.run(&old, &source, &d.path().join("Stale"), |_| {})
            .is_err()
    );
    let r = jobs
        .run(&current, &source, &d.path().join("Good"), |_| {})
        .unwrap();
    assert!(!r.cancelled);
    assert!(r.path.unwrap().is_file());
    assert!(!d.path().join("Stale").exists());
}

#[test]
fn failed_jobs_release_the_slot_and_never_open_or_modify_the_source() {
    let jobs = RecoveryJobs::default();
    let d = tempfile::tempdir().unwrap();
    let id = jobs.begin().unwrap();
    assert!(
        jobs.run(
            &id,
            &d.path().join("Missing.2cbh"),
            &d.path().join("Output"),
            |_| {}
        )
        .is_err()
    );
    assert!(!d.path().join("Output").exists());
    assert!(jobs.begin().is_ok());
}
