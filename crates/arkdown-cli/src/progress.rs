use std::time::Instant;

pub struct Progress {
    step_start: std::cell::Cell<Instant>,
}

impl Progress {
    pub fn new() -> Self {
        Progress {
            step_start: std::cell::Cell::new(Instant::now()),
        }
    }

    pub fn step(&self, n: u32, total: u32, label: &str) {
        self.step_start.set(Instant::now());
        eprint!("  [{}/{}] {}...", n, total, label);
    }

    pub fn done(&self) {
        let ms = self.step_start.get().elapsed().as_millis();
        eprintln!("  done ({ms}ms)");
    }
}
