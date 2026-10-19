use std::time::Instant;

pub struct Progress {
    step_start: Instant,
    active: Option<(u32, u32, String)>,
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

impl Progress {
    pub fn new() -> Self {
        Progress {
            step_start: Instant::now(),
            active: None,
        }
    }

    pub fn step(&mut self, n: u32, total: u32, label: &str) {
        self.step_start = Instant::now();
        self.active = Some((n, total, label.to_owned()));
        tracing::info!(
          target: "arkdown_build::coordinator",
          step = n,
          total,
          label,
          source = "rust_stage",
          phase = "start",
          "build step started"
        );
    }

    pub fn done(&mut self) {
        let Some((step, total, label)) = self.active.take() else {
            return;
        };
        let ms = self.step_start.elapsed().as_secs_f64() * 1000.0;
        tracing::info!(
          target: "arkdown_build::coordinator",
          elapsed_ms = ms,
          step, total, label,
          source = "rust_stage",
          phase = "end",
          "build step completed"
        );
    }
}
