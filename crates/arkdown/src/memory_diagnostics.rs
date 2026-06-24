//! Optional memory diagnostics for build-time profiling.
//!
//! This module is compiled only with the `memory-diagnostics` feature. It is
//! intentionally outside plugin business logic and writes no files unless
//! `ARKDOWN_MEMORY_DUMP` is set.

use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::json;

static START: OnceLock<Instant> = OnceLock::new();

/// Captured process memory at one point in the build.
#[derive(Debug, Clone, Copy)]
pub struct MemorySnapshot {
    /// Resident memory reported by the OS.
    pub physical_bytes: usize,
    /// Virtual memory reported by the OS.
    pub virtual_bytes: usize,
}

impl MemorySnapshot {
    fn current() -> Option<Self> {
        memory_stats::memory_stats().map(|stats| Self {
            physical_bytes: stats.physical_mem,
            virtual_bytes: stats.virtual_mem,
        })
    }

    fn physical_mib(self) -> f64 {
        bytes_to_mib(self.physical_bytes)
    }

    fn virtual_mib(self) -> f64 {
        bytes_to_mib(self.virtual_bytes)
    }
}

/// Capture the current process memory without emitting a diagnostic record.
pub fn snapshot() -> Option<MemorySnapshot> {
    MemorySnapshot::current()
}

/// Emit a memory point with no extra detail.
pub fn dump(label: &str) {
    dump_with(label, format_args!(""));
}

/// Emit a memory point to tracing and optionally to a JSON-lines dump file.
pub fn dump_with(label: &str, details: fmt::Arguments<'_>) {
    let details = details.to_string();
    let elapsed_ms = START.get_or_init(Instant::now).elapsed().as_millis();
    let Some(snapshot) = MemorySnapshot::current() else {
        tracing::info!(
            target: "arkdown::memory",
            label,
            elapsed_ms,
            details = %details,
            "memory diagnostic unavailable"
        );
        return;
    };
    tracing::info!(
        target: "arkdown::memory",
        label,
        elapsed_ms,
        rss_bytes = snapshot.physical_bytes,
        vmem_bytes = snapshot.virtual_bytes,
        rss_mib = snapshot.physical_mib(),
        vmem_mib = snapshot.virtual_mib(),
        details = %details,
        "memory diagnostic"
    );
    append_dump_file(label, elapsed_ms, snapshot, &details);
}

/// Starts a background process-memory sampler when
/// `ARKDOWN_MEMORY_SAMPLE_MS` is set.
pub fn start_sampler_from_env() -> Option<MemorySamplerGuard> {
    let interval_ms = std::env::var("ARKDOWN_MEMORY_SAMPLE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|interval| *interval > 0)?;
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let interval = Duration::from_millis(interval_ms);
    let handle = std::thread::Builder::new()
        .name("arkdown-memory-sampler".to_string())
        .spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                sample("sample", "");
                std::thread::sleep(interval);
            }
        })
        .ok()?;
    tracing::info!(
        target: "arkdown::memory",
        interval_ms,
        "memory sampler enabled"
    );
    Some(MemorySamplerGuard {
        stop,
        handle: Some(handle),
    })
}

/// Guard that stops the optional background memory sampler on drop.
#[derive(Debug)]
pub struct MemorySamplerGuard {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for MemorySamplerGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn sample(label: &str, details: &str) {
    let elapsed_ms = START.get_or_init(Instant::now).elapsed().as_millis();
    if let Some(snapshot) = MemorySnapshot::current() {
        append_dump_file(label, elapsed_ms, snapshot, details);
    }
}

fn append_dump_file(label: &str, elapsed_ms: u128, snapshot: MemorySnapshot, details: &str) {
    let Some(path) = dump_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if let Err(error) = std::fs::create_dir_all(parent) {
        tracing::warn!(target: "arkdown::memory", error = %error, path = %path.display(), "failed to create memory dump directory");
        return;
    }
    let record = json!({
        "elapsed_ms": elapsed_ms,
        "label": label,
        "rss_bytes": snapshot.physical_bytes,
        "rss_mib": snapshot.physical_mib(),
        "vmem_bytes": snapshot.virtual_bytes,
        "vmem_mib": snapshot.virtual_mib(),
        "details": details,
    });
    let line = match serde_json::to_string(&record) {
        Ok(line) => line,
        Err(error) => {
            tracing::warn!(target: "arkdown::memory", error = %error, "failed to serialize memory dump record");
            return;
        }
    };
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(error) = writeln!(file, "{line}") {
                tracing::warn!(target: "arkdown::memory", error = %error, path = %path.display(), "failed to write memory dump record");
            }
        }
        Err(error) => {
            tracing::warn!(target: "arkdown::memory", error = %error, path = %path.display(), "failed to open memory dump file");
        }
    }
}

fn dump_path() -> Option<PathBuf> {
    std::env::var_os("ARKDOWN_MEMORY_DUMP")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn bytes_to_mib(bytes: usize) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}
