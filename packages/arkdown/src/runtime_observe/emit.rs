use tracing::Level;

use super::observability::RuntimeObservability;

#[derive(Debug, Clone)]
pub struct RuntimeLogRecord {
  pub level: String,
  pub category: String,
  pub message: String,
  pub duration_id: Option<String>,
  pub task_path: Option<String>,
  pub event_id: Option<String>,
  pub source: String,
}

pub(crate) struct RuntimeObservabilityEmitter<'a> {
  observability: &'a RuntimeObservability,
}

impl<'a> RuntimeObservabilityEmitter<'a> {
  pub(crate) fn new(observability: &'a RuntimeObservability) -> Self {
    Self { observability }
  }

  pub(crate) fn emit_log(&self, record: &RuntimeLogRecord) {
    let level = level_from_name(&record.level);
    let build_id = self.observability.build_id();
    let category = record.category.as_str();
    let duration_id = record.duration_id.as_deref();
    let task_path = record.task_path.as_deref();
    let event_id = record.event_id.as_deref();
    let source = record.source.as_str();
    let message = record.message.as_str();
    match level {
      Level::ERROR => tracing::error!(
        target: "arkdown_napi::hvigor",
        build_id = %build_id,
        category,
        duration_id,
        task_path,
        event_id,
        source,
        "{message}"
      ),
      Level::WARN => tracing::warn!(
        target: "arkdown_napi::hvigor",
        build_id = %build_id,
        category,
        duration_id,
        task_path,
        event_id,
        source,
        "{message}"
      ),
      Level::INFO => tracing::info!(
        target: "arkdown_napi::hvigor",
        build_id = %build_id,
        category,
        duration_id,
        task_path,
        event_id,
        source,
        "{message}"
      ),
      Level::DEBUG => tracing::debug!(
        target: "arkdown_napi::hvigor",
        build_id = %build_id,
        category,
        duration_id,
        task_path,
        event_id,
        source,
        "{message}"
      ),
      Level::TRACE => tracing::trace!(
        target: "arkdown_napi::hvigor",
        build_id = %build_id,
        category,
        duration_id,
        task_path,
        event_id,
        source,
        "{message}"
      ),
    }
  }
}

fn level_from_name(name: &str) -> Level {
  match name.trim().to_ascii_lowercase().as_str() {
    "error" | "err" | "fatal" => Level::ERROR,
    "warn" | "warning" => Level::WARN,
    "debug" | "trace" => Level::DEBUG,
    _ => Level::INFO,
  }
}
