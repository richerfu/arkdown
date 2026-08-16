use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use napi::bindgen_prelude::Function;
use napi::threadsafe_function::ThreadsafeCallContext;
use napi_derive::napi;
use serde::Serialize;
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior};

use crate::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt, MaybeAsyncJsReturn};

use super::observability::RuntimeObservability;

const REPORT_INTERVAL: Duration = Duration::from_secs(1);
const BYTES_PER_MEBIBYTE: f64 = 1024.0 * 1024.0;

type ProcessMemorySampler = MaybeAsyncJsCallback<(), (), JsProcessMemoryUsage>;

#[napi(object)]
pub struct JsProcessMemoryUsage {
  pub rss: f64,
  pub heap_total: f64,
  pub heap_used: f64,
  pub external: f64,
  pub array_buffers: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessMemorySample {
  time: u64,
  rss: f64,
  heap_total: f64,
  heap_used: f64,
  external: f64,
  array_buffers: f64,
}

impl ProcessMemorySample {
  fn capture(memory: JsProcessMemoryUsage) -> Self {
    Self {
      time: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64,
      rss: memory.rss / BYTES_PER_MEBIBYTE,
      heap_total: memory.heap_total / BYTES_PER_MEBIBYTE,
      heap_used: memory.heap_used / BYTES_PER_MEBIBYTE,
      external: memory.external / BYTES_PER_MEBIBYTE,
      array_buffers: memory.array_buffers / BYTES_PER_MEBIBYTE,
    }
  }
}

#[derive(Serialize)]
struct ProcessMonitorReport<'samples> {
  version: &'static str,
  pid: u32,
  data: &'samples [ProcessMemorySample],
}

/// Rust-owned equivalent of Hvigor 6.24.x `ReportServiceImpl` process
/// monitoring. JavaScript supplies only the live V8 memory snapshot because
/// those counters are not available from the Rust process APIs.
#[napi]
pub struct RuntimeProcessMonitor {
  sampler: ProcessMemorySampler,
  pid: u32,
  samples: Arc<Mutex<Vec<ProcessMemorySample>>>,
  task: Option<JoinHandle<()>>,
}

#[napi]
impl RuntimeProcessMonitor {
  #[napi(constructor)]
  pub fn new(
    sampler: Function<(), MaybeAsyncJsReturn<JsProcessMemoryUsage>>,
    pid: u32,
  ) -> napi::Result<Self> {
    let sampler = sampler
      .build_threadsafe_function::<()>()
      .weak::<true>()
      .build_callback(|context: ThreadsafeCallContext<()>| Ok(context.value))?;
    Ok(Self {
      sampler: Arc::new(sampler),
      pid,
      samples: Arc::new(Mutex::new(Vec::new())),
      task: None,
    })
  }

  #[napi]
  pub fn start(&mut self) -> napi::Result<()> {
    self.abort_task();
    self
      .samples
      .lock()
      .map_err(|_| napi::Error::from_reason("runtime process monitor sample lock was poisoned"))?
      .clear();

    let sampler = Arc::clone(&self.sampler);
    let samples = Arc::clone(&self.samples);
    self.task = Some(napi::bindgen_prelude::spawn(async move {
      let mut interval =
        tokio::time::interval_at(Instant::now() + REPORT_INTERVAL, REPORT_INTERVAL);
      interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
      loop {
        interval.tick().await;
        let memory = match sampler.await_call(()).await {
          Ok(memory) => memory,
          Err(error) => {
            tracing::debug!(
              target: "arkdown_napi::metric",
              error = %error,
              "runtime process memory sampler stopped"
            );
            break;
          }
        };
        let Ok(mut records) = samples.lock() else {
          tracing::warn!(
            target: "arkdown_napi::metric",
            "runtime process monitor sample lock was poisoned"
          );
          break;
        };
        records.push(ProcessMemorySample::capture(memory));
      }
    }));
    Ok(())
  }

  #[napi]
  pub fn stop(&mut self) -> napi::Result<Option<String>> {
    if self.task.is_none() {
      return Ok(None);
    }
    self.abort_task();
    let samples = self
      .samples
      .lock()
      .map_err(|_| napi::Error::from_reason("runtime process monitor sample lock was poisoned"))?;
    let report = serde_json::to_string(&ProcessMonitorReport {
      version: "1.0",
      pid: self.pid,
      data: &samples,
    })
    .map_err(|error| {
      napi::Error::from_reason(format!("serialize runtime process monitor: {error}"))
    })?;
    RuntimeObservability::global()
      .ok_or_else(|| napi::Error::from_reason("runtime observability is not installed"))?
      .persist_monitor(&report)
      .map(|path| Some(path.to_string_lossy().into_owned()))
      .map_err(|error| napi::Error::from_reason(error.to_string()))
  }
}

impl RuntimeProcessMonitor {
  fn abort_task(&mut self) {
    if let Some(task) = self.task.take() {
      task.abort();
    }
  }
}

impl Drop for RuntimeProcessMonitor {
  fn drop(&mut self) {
    self.abort_task();
  }
}
