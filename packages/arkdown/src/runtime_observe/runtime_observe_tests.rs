use std::path::PathBuf;

use tempfile::tempdir;
use tracing::Level;

use std::sync::Arc;

use super::emit::RuntimeLogRecord;
use super::observability::ObservabilityDirectiveSet;
use super::observability::{
  ConsoleOwner, ObservabilityInstallPlan, ObservabilityLevel, ObservabilityMode,
  RuntimeObservability,
};
use super::persist::BuildLogWriter;

#[test]
fn directive_set_turns_js_targets_off_and_keeps_coordinator_warn() {
  let filter = ObservabilityDirectiveSet::parse(
    "arkdown=warn,arkdown_napi=warn,arkdown_napi::hvigor=off,arkdown_napi::metric=off",
  );
  assert!(!filter.enabled("arkdown_napi::hvigor", &Level::WARN));
  assert!(!filter.enabled("arkdown_napi::metric", &Level::INFO));
  assert!(filter.enabled("arkdown_napi::cmd::build::coordinator", &Level::WARN));
  assert!(!filter.enabled("arkdown_napi::cmd::build::coordinator", &Level::INFO));
  assert!(filter.enabled("arkdown", &Level::ERROR));
}

#[test]
fn directive_set_off_drops_every_target() {
  let filter = ObservabilityDirectiveSet::parse("off");
  assert!(!filter.enabled("arkdown_napi::hvigor", &Level::ERROR));
  assert!(!filter.enabled("arkdown_napi::cmd::build::coordinator", &Level::INFO));
}

#[test]
fn directive_set_file_info_keeps_hvigor_and_coordinator() {
  let filter = ObservabilityDirectiveSet::parse("arkdown=info,arkdown_napi=info");
  assert!(filter.enabled("arkdown_napi::hvigor", &Level::INFO));
  assert!(filter.enabled("arkdown_napi::cmd::build::coordinator", &Level::INFO));
  assert!(!filter.enabled("arkdown_napi::cmd::build::coordinator", &Level::DEBUG));
}

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
  writer.flush().expect("flush");
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

#[test]
fn install_napi_and_emit_log_writes_build_log() {
  let directory = tempdir().expect("temp dir");
  let cache_dir = directory.path().join(".hvigor");
  let observability = RuntimeObservability::install_napi(ObservabilityInstallPlan {
    mode: ObservabilityMode::Napi,
    console_owner: ConsoleOwner::Log4js,
    cache_dir: cache_dir.clone(),
    build_id: Arc::from("222222222222222222"),
    log_level: ObservabilityLevel::Info,
  })
  .expect("install");
  assert!(observability.has_active_napi_plan());
  RuntimeObservability::global()
    .expect("global")
    .emit_log(&RuntimeLogRecord {
      level: "info".to_string(),
      category: "fixture".to_string(),
      message: "hello-observe".to_string(),
      duration_id: None,
      task_path: None,
      event_id: None,
      source: "js_plugin".to_string(),
    });
  observability.finish().expect("flush build log");
  assert!(!observability.has_active_napi_plan());
  let content = std::fs::read_to_string(
    cache_dir
      .join("outputs")
      .join("build-logs")
      .join("build.log"),
  )
  .expect("read build log");
  assert!(
    content.contains("hello-observe"),
    "build.log must contain the emitted message, got {content:?}"
  );
}
