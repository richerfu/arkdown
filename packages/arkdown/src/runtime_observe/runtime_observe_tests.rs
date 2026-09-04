use std::path::PathBuf;

use super::persist::BuildLogWriter;
use tempfile::tempdir;

#[test]
fn build_log_writer_rotates_after_size_limit() {
  let directory = tempdir().expect("temp dir");
  let writer = BuildLogWriter::new();
  writer.reopen(directory.path()).expect("open build log");
  let chunk = vec![b'x'; 1_048_576];
  writer.write_line_bytes(&chunk).expect("write first");
  writer.write_line_bytes(&chunk).expect("write second");
  writer
    .write_line_bytes(b" after-rotate\n")
    .expect("write third");
  drop(writer);
  let active = directory
    .path()
    .join("outputs")
    .join("build-logs")
    .join("build.log");
  let backup = PathBuf::from(format!("{}.1", active.display()));
  assert!(active.exists(), "active log must exist after rotation");
  assert!(backup.exists(), "rotated backup must exist");
  let active_len = std::fs::metadata(&active).expect("active metadata").len();
  assert!(active_len < 2_097_152, "active log must be the new file");
}

#[test]
fn build_log_writer_is_inert_before_reopen() {
  let writer = BuildLogWriter::new();
  writer
    .write_line("should be dropped\n")
    .expect("inert write");
}
