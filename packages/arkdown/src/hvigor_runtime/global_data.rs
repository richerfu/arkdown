use std::sync::{Mutex, OnceLock};

use napi_derive::napi;
use time::OffsetDateTime;

struct RuntimeHvigorBuildIdSequence {
  timestamp: String,
  counter: u64,
}

/// Rust-owned build identity behind Hvigor's undocumented `globalData` export.
/// The JavaScript facade retains `cliEnv` and `cliOpts` only because callers
/// expect their exact V8 object identities.
#[napi]
#[derive(Default)]
pub struct RuntimeHvigorGlobalData {
  build_id: Option<String>,
}

#[napi]
impl RuntimeHvigorGlobalData {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self::default()
  }

  #[napi]
  pub fn init(&mut self) -> String {
    let timestamp = Self::local_timestamp();
    // Hvigor appends a process-wide collision counter when multiple build IDs
    // are created in the same millisecond, so this tiny critical section is
    // required to preserve uniqueness and deterministic allocation order.
    let sequence = Self::sequence();
    let mut sequence = sequence
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    if sequence.timestamp != timestamp {
      sequence.timestamp.clone_from(&timestamp);
      sequence.counter = 0;
    } else {
      sequence.counter = sequence.counter.saturating_add(1);
    }
    let build_id = format!("{timestamp}{}", sequence.counter);
    self.build_id = Some(build_id.clone());
    build_id
  }

  #[napi]
  pub fn clean(&mut self) {
    self.build_id = None;
  }

  #[napi(getter)]
  pub fn build_id(&self) -> Option<String> {
    self.build_id.clone()
  }
}

impl RuntimeHvigorGlobalData {
  fn sequence() -> &'static Mutex<RuntimeHvigorBuildIdSequence> {
    static SEQUENCE: OnceLock<Mutex<RuntimeHvigorBuildIdSequence>> = OnceLock::new();
    SEQUENCE.get_or_init(|| {
      Mutex::new(RuntimeHvigorBuildIdSequence {
        timestamp: String::new(),
        counter: 0,
      })
    })
  }

  fn local_timestamp() -> String {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!(
      "{:04}{:02}{:02}{:02}{:02}{:02}{:03}",
      now.year(),
      u8::from(now.month()),
      now.day(),
      now.hour(),
      now.minute(),
      now.second(),
      now.millisecond()
    )
  }
}
