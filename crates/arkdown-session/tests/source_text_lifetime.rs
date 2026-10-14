use arkdown_core::{SourceInventory, SourceReadError};
use arkdown_session::SourceStore;

#[test]
fn retiring_texts_preserves_snapshot_identity_and_rejects_all_content_reads() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.ts");
    std::fs::write(&source, "export const captured = 1;").unwrap();
    let store = SourceStore::discover(&[temp.path().into()]).unwrap();
    let fingerprint = store.fingerprint(&source).unwrap().clone();
    let file = store.source_by_path(&source).unwrap().clone();
    let lease = store.read_text(&source).unwrap();
    store.release_texts();
    store.release_texts();
    std::fs::write(&source, "export const changed = 2;").unwrap();

    assert_eq!(store.fingerprint(&source), Some(&fingerprint));
    assert_eq!(store.canonical_path(&source), Some(file.path.as_path()));
    assert_eq!(lease.as_ref(), "export const captured = 1;");
    assert_eq!(store.source_files_under(temp.path()).len(), 1);
    assert!(matches!(
        store.read_text(&source),
        Err(SourceReadError::Released { .. })
    ));
    assert!(matches!(
        store.read_bytes(&source),
        Err(SourceReadError::Released { .. })
    ));
    assert!(matches!(
        store.source_text(&source),
        Err(SourceReadError::Released { .. })
    ));
    assert!(matches!(
        store.text_sources_under(temp.path()),
        Err(SourceReadError::Released { .. })
    ));
    assert!(matches!(
        file.cached_content(),
        Err(SourceReadError::Released { .. })
    ));
    assert!(store
        .source_text(&temp.path().join("missing.ts"))
        .unwrap()
        .is_none());
}

#[test]
fn retained_watch_snapshot_keeps_original_bytes_when_a_new_snapshot_is_discovered() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.ts");
    std::fs::write(&source, "first").unwrap();
    let old = SourceStore::discover(&[temp.path().into()]).unwrap();
    std::fs::write(&source, "second").unwrap();
    let new = SourceStore::discover(&[temp.path().into()]).unwrap();
    assert_eq!(old.read_text(&source).unwrap().as_ref(), "first");
    assert_eq!(new.read_text(&source).unwrap().as_ref(), "second");
    new.release_texts();
    assert_eq!(old.read_text(&source).unwrap().as_ref(), "first");
}

#[test]
fn eviction_keeps_leased_texts_and_verified_reads_reject_changed_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let used = temp.path().join("used.ts");
    let unused = temp.path().join("unused.ts");
    let empty = temp.path().join("empty.ts");
    std::fs::write(&used, "first").unwrap();
    std::fs::write(&unused, "unused").unwrap();
    std::fs::write(&empty, "").unwrap();
    let store = SourceStore::discover(&[temp.path().into()]).unwrap();
    let lease = store.read_text(&used).unwrap();
    let fingerprint = store.fingerprint(&unused).unwrap().clone();
    let evicted = store.evict_unleased_texts();
    assert_eq!((evicted.files, evicted.bytes), (2, 6));
    std::fs::write(&used, "later").unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &store.read_text(&used).unwrap(),
        &lease
    ));
    // Same length: content hash, not just size, must be checked.
    std::fs::write(&unused, "change").unwrap();
    let error = store.read_text(&unused).unwrap_err();
    assert!(error
        .to_string()
        .contains("ARKDOWN_SOURCE_SNAPSHOT_MISMATCH"));
    assert!(
        matches!(error, SourceReadError::SnapshotMismatch { path, .. } if path.ends_with("unused.ts"))
    );
    assert_eq!(store.fingerprint(&unused), Some(&fingerprint));
    std::fs::write(&unused, "unused").unwrap();
    // Includes an empty file, whose restored file count must not disappear.
    assert_eq!(store.restore_evicted_texts().unwrap(), evicted);
    assert_eq!(store.read_bytes(&unused).unwrap().as_slice(), b"unused");
}

#[test]
fn parallel_restore_reports_the_first_changed_path_deterministically() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["a.ts", "z.ts"] {
        std::fs::write(temp.path().join(name), "startup").unwrap();
    }
    let store = SourceStore::discover(&[temp.path().into()]).unwrap();
    store.evict_unleased_texts();
    for name in ["a.ts", "z.ts"] {
        std::fs::write(temp.path().join(name), "changed and longer").unwrap();
    }
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let error = pool.install(|| store.restore_evicted_texts()).unwrap_err();
        assert!(
            matches!(error, SourceReadError::SnapshotMismatch { path, .. } if path.ends_with("a.ts"))
        );
    }
}

#[test]
fn reload_io_errors_are_typed_and_release_permanently_retires_evicted_texts() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.ts");
    std::fs::write(&source, "startup").unwrap();
    let store = SourceStore::discover(&[temp.path().into()]).unwrap();
    store.evict_unleased_texts();
    std::fs::remove_file(&source).unwrap();
    let error = store.read_text(&source).unwrap_err();
    assert!(error.to_string().contains("ARKDOWN_SOURCE_RELOAD_IO"));
    assert!(matches!(error, SourceReadError::ReloadIo { path, .. } if path.ends_with("source.ts")));
    assert!(matches!(
        store.restore_evicted_texts(),
        Err(SourceReadError::ReloadIo { .. })
    ));
    store.release_texts();
    std::fs::write(&source, "startup").unwrap();
    assert_eq!(store.restore_evicted_texts().unwrap().files, 0);
    assert!(matches!(
        store.read_text(&source),
        Err(SourceReadError::Released { .. })
    ));
}
