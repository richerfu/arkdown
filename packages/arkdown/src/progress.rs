use std::time::Instant;

pub struct Progress {
  step_start: std::cell::Cell<Instant>,
}

impl Default for Progress {
  fn default() -> Self {
    Self::new()
  }
}

impl Progress {
  pub fn new() -> Self {
    Progress {
      step_start: std::cell::Cell::new(Instant::now()),
    }
  }

  pub fn step(&self, n: u32, total: u32, label: &str) {
    self.step_start.set(Instant::now());
    match crate::runtime_observe::RuntimeObservability::global() {
      Some(observability) => {
        let build_id = observability.build_id();
        tracing::info!(
          target: "arkdown_napi::services::build::coordinator",
          build_id = %build_id,
          step = n,
          total,
          label,
          source = "rust_stage",
          "build step started"
        );
      }
      None => {
        tracing::info!(
          target: "arkdown_napi::services::build::coordinator",
          step = n,
          total,
          label,
          source = "rust_stage",
          "build step started"
        );
      }
    }
  }

  pub fn done(&self) {
    let ms = self.step_start.get().elapsed().as_millis();
    match crate::runtime_observe::RuntimeObservability::global() {
      Some(observability) => {
        let build_id = observability.build_id();
        tracing::info!(
          target: "arkdown_napi::services::build::coordinator",
          build_id = %build_id,
          elapsed_ms = ms,
          source = "rust_stage",
          "build step completed"
        );
      }
      None => {
        tracing::info!(
          target: "arkdown_napi::services::build::coordinator",
          elapsed_ms = ms,
          source = "rust_stage",
          "build step completed"
        );
      }
    }
  }
}
