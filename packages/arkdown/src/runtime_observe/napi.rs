use std::path::PathBuf;
use std::sync::Arc;

use napi_derive::napi;

use super::emit::RuntimeLogRecord as NativeLogRecord;
use super::observability::{
  ConsoleOwner, ObservabilityInstallPlan, ObservabilityLevel, ObservabilityMode,
  RuntimeObservability,
};

#[napi(object)]
pub struct ObservabilityInstallOptions {
  pub mode: String,
  pub cache_dir: String,
  pub build_id: String,
  pub log_level: String,
  pub console_owner: String,
}

#[napi(object)]
pub struct RuntimeLogRecord {
  pub level: String,
  pub category: String,
  pub message: String,
  pub duration_id: Option<String>,
  pub task_path: Option<String>,
  pub event_id: Option<String>,
  pub source: String,
}

#[napi]
pub fn install_runtime_observability(options: ObservabilityInstallOptions) -> napi::Result<String> {
  let plan = ObservabilityInstallPlan {
    mode: if options.mode == "cli" {
      ObservabilityMode::Cli
    } else {
      ObservabilityMode::Napi
    },
    console_owner: if options.console_owner == "tracing" {
      ConsoleOwner::Tracing
    } else {
      ConsoleOwner::Log4js
    },
    cache_dir: PathBuf::from(options.cache_dir),
    build_id: Arc::from(options.build_id),
    log_level: ObservabilityLevel::parse(&options.log_level),
  };
  RuntimeObservability::install_napi(plan)
    .map(|installed| installed.build_id().to_string())
    .map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[napi]
pub fn runtime_observability_is_enabled(level: String, target: String) -> bool {
  RuntimeObservability::global()
    .map(|observability| observability.is_enabled(ObservabilityLevel::parse(&level), &target))
    .unwrap_or(false)
}

#[napi]
pub fn emit_runtime_log(record: RuntimeLogRecord) {
  if let Some(observability) = RuntimeObservability::global() {
    observability.emit_log(&NativeLogRecord {
      level: record.level,
      category: record.category,
      message: record.message,
      duration_id: record.duration_id,
      task_path: record.task_path,
      event_id: record.event_id,
      source: record.source,
    });
  }
}

#[napi]
pub fn finish_runtime_observability() -> napi::Result<()> {
  if let Some(observability) = RuntimeObservability::global() {
    observability
      .finish()
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
  }
  Ok(())
}

#[napi]
pub fn runtime_hvigor_report_snapshot(ppid: u32) -> serde_json::Value {
  RuntimeObservability::hvigor_report_snapshot(ppid)
}

#[napi]
pub fn persist_runtime_report(report_json: String, analyze_html: bool) -> napi::Result<String> {
  RuntimeObservability::global()
    .ok_or_else(|| napi::Error::from_reason("runtime observability is not installed"))?
    .persist_report(&report_json, analyze_html)
    .map(|path| path.to_string_lossy().into_owned())
    .map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[napi]
pub fn persist_runtime_details(details_json: String) -> napi::Result<String> {
  RuntimeObservability::global()
    .ok_or_else(|| napi::Error::from_reason("runtime observability is not installed"))?
    .persist_details(&details_json)
    .map(|path| path.to_string_lossy().into_owned())
    .map_err(|error| napi::Error::from_reason(error.to_string()))
}
