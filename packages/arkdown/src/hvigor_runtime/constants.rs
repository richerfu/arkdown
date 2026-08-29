use napi_derive::napi;

/// Rust-owned value plan for the declared worker API and the small set of
/// task constants required by Arkdown's existing custom-task compatibility.
#[napi]
#[derive(Default)]
pub struct RuntimeHvigorConstantApi;

#[napi]
impl RuntimeHvigorConstantApi {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self
  }

  #[napi]
  pub fn snapshot(&self) -> serde_json::Value {
    serde_json::json!({
      "priority": {
        "0": "FIRST",
        "1": "HEAVY",
        "2": "MEDIUM",
        "3": "LIGHT",
        "4": "LAST",
        "FIRST": 0,
        "HEAVY": 1,
        "MEDIUM": 2,
        "LIGHT": 3,
        "LAST": 4
      },
      "taskState": {
        "WAITING": "waiting",
        "RUNNING": "running",
        "END": "end",
        "REJECT": "reject",
        "ERROR": "error"
      },
      "worker": {
        "customWorkerRequest": "CUSTOM_WORKER_REQUEST",
        "workDone": "work done",
        "workError": "work error"
      }
    })
  }
}
