use arkdown::HvigorHarImportees;

use super::har_importee_cache::HarImporteeCache;

#[test]
fn writes_exact_hvigor_importee_files_for_optimized_bundled_bytecode_har() {
  let temp = tempfile::tempdir().expect("create temp dir");
  let cache = HarImporteeCache::new(temp.path());
  let importees = HvigorHarImportees {
    non_entry: vec!["source-har/feature".to_string()],
    compiled: vec!["source-har".to_string(), "npm-package/subpath".to_string()],
    direct: vec!["source-har".to_string()],
  };

  cache
    .write_compile_output(&importees, true, true, true)
    .expect("write importee cache");

  assert_eq!(
    std::fs::read(temp.path().join("non.entry.importee.json")).expect("read non-entry"),
    b"[\"source-har/feature\"]\n"
  );
  assert_eq!(
    std::fs::read(temp.path().join("compiled.importees.json")).expect("read compiled"),
    b"[\"source-har\",\"npm-package/subpath\"]\n"
  );
  assert_eq!(
    std::fs::read(temp.path().join("direct.importees.json")).expect("read direct"),
    b"[\"source-har\"]\n"
  );
  assert_eq!(cache.read().expect("read importee cache"), importees);
}

#[test]
fn inactive_compile_does_not_create_importee_cache_files() {
  let temp = tempfile::tempdir().expect("create temp dir");
  let cache = HarImporteeCache::new(temp.path().join("cache"));
  cache
    .write_compile_output(&HvigorHarImportees::default(), false, true, false)
    .expect("inactive write");
  assert!(!temp.path().join("cache").exists());
}
