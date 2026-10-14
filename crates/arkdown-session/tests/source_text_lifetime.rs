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
