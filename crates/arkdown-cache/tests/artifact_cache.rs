use arkdown_cache::{ArtifactCacheStore, ArtifactInputHasher};

#[test]
fn stage_completion_requires_valid_content_and_survives_only_successful_publication() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("signed.hap");
    let cache = root.path().join("complete.json");
    let store = ArtifactCacheStore::new(cache.clone());
    let outputs = vec![output.clone()];
    assert!(!store.matches("inputs-a", &outputs).unwrap());
    assert!(store.commit("inputs-a", &outputs).is_err());
    std::fs::write(&output, b"artifact-a").unwrap();
    store.commit("inputs-a", &outputs).unwrap();
    assert!(store.matches("inputs-a", &outputs).unwrap());
    assert!(!store.matches("inputs-b", &outputs).unwrap());
    std::fs::write(&output, b"artifact-b").unwrap();
    assert!(
        !store.matches("inputs-a", &outputs).unwrap(),
        "same length corruption must miss"
    );
    store.invalidate().unwrap();
    assert!(
        !store.matches("inputs-a", &outputs).unwrap(),
        "failed execution must not publish completion"
    );
    store.commit("inputs-b", &outputs).unwrap();
    std::fs::remove_file(&output).unwrap();
    assert!(!store.matches("inputs-b", &outputs).unwrap());
    std::fs::write(&cache, b"{partial commit").unwrap();
    assert!(!store.matches("inputs-b", &outputs).unwrap());
}

#[test]
fn file_fingerprints_detect_same_size_edits_and_missing_to_present_transitions() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("profile");
    let fingerprint = || {
        let mut hash = ArtifactInputHasher::new("sign");
        hash.file(&input).unwrap();
        hash.finish()
    };
    let missing = fingerprint();
    std::fs::write(&input, b"first").unwrap();
    let first = fingerprint();
    std::fs::write(&input, b"other").unwrap();
    assert_ne!(first, fingerprint());
    assert_ne!(missing, first);
}

#[test]
fn parallel_inventory_matches_serial_fingerprints_at_different_thread_counts() {
    let root = tempfile::tempdir().unwrap();
    let files = (0..20)
        .map(|index| {
            let path = root.path().join(format!("native-{index}.so"));
            std::fs::write(&path, vec![index as u8; 150_000 + index]).unwrap();
            path
        })
        .chain(std::iter::once(root.path().join("missing")))
        .collect::<Vec<_>>();
    let paths = files.iter().map(|path| path.as_path()).collect::<Vec<_>>();
    let mut serial = ArtifactInputHasher::new("package");
    for path in &paths {
        serial.file(path).unwrap();
    }
    let expected = serial.finish();
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let actual = pool.install(|| {
            let mut hash = ArtifactInputHasher::new("package");
            hash.files(&paths).unwrap();
            hash.finish()
        });
        assert_eq!(actual, expected);
    }
}
