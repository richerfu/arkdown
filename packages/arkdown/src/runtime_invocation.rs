use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use napi::bindgen_prelude::*;
use napi::threadsafe_function::ThreadsafeCallContext;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

use crate::js_callback::{MaybeAsyncJsCallbackExt, MaybeAsyncJsReturn, StrongMaybeAsyncJsCallback};

static ACTIVE_INVOCATION: Mutex<Option<RuntimeInvocationRecord>> = Mutex::new(None);
static INVOCATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct RuntimeInvocationRecord {
  token: String,
  project_root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeInvocationError {
  #[error(
    "ARKDOWN_CONCURRENT_RUNTIME_INVOCATION: project {requested} cannot start while project {active} owns the process-wide Hvigor JavaScript runtime"
  )]
  Concurrent { requested: PathBuf, active: PathBuf },
  #[error(
    "ARKDOWN_RUNTIME_INVOCATION_NOT_ACTIVE: no Hvigor JavaScript runtime invocation is active"
  )]
  NotActive,
  #[error("ARKDOWN_RUNTIME_INVOCATION_TOKEN_MISMATCH: active Hvigor JavaScript runtime token does not match {0}")]
  TokenMismatch(String),
  #[error("ARKDOWN_RUNTIME_INVOCATION_LOCK_POISONED: process-wide Hvigor JavaScript runtime lock was poisoned")]
  LockPoisoned,
}

pub(crate) struct RuntimeInvocationLease;

impl RuntimeInvocationLease {
  pub(crate) fn acquire(
    project_root: &Path,
  ) -> std::result::Result<String, RuntimeInvocationError> {
    let project_root = arkdown_path::lexical_normalize(project_root);
    let mut active = ACTIVE_INVOCATION
      .lock()
      .map_err(|_| RuntimeInvocationError::LockPoisoned)?;
    if let Some(record) = active.as_ref() {
      return Err(RuntimeInvocationError::Concurrent {
        requested: project_root,
        active: record.project_root.clone(),
      });
    }
    let sequence = INVOCATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let token = format!("{}-{sequence}", std::process::id());
    *active = Some(RuntimeInvocationRecord {
      token: token.clone(),
      project_root,
    });
    Ok(token)
  }

  pub(crate) fn release(token: &str) -> std::result::Result<(), RuntimeInvocationError> {
    let mut active = ACTIVE_INVOCATION
      .lock()
      .map_err(|_| RuntimeInvocationError::LockPoisoned)?;
    let Some(record) = active.as_ref() else {
      return Err(RuntimeInvocationError::NotActive);
    };
    if record.token != token {
      return Err(RuntimeInvocationError::TokenMismatch(token.to_string()));
    }
    *active = None;
    Ok(())
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandExecutionPlan {
  #[serde(default)]
  tasks: Vec<RuntimeCommandExecutionTask>,
  #[serde(default)]
  native_requests: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandExecutionTask {
  task_path: String,
  execution_lane: RuntimeCommandExecutionLane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RuntimeCommandExecutionLane {
  Native,
  Standalone,
  SyncCustom,
  Custom,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandExecutionReport {
  artifacts: Vec<String>,
  output: Vec<String>,
}

type RuntimeStandaloneArguments = (String,);
type RuntimeStandaloneCallback = StrongMaybeAsyncJsCallback<
  RuntimeStandaloneArguments,
  FnArgs<RuntimeStandaloneArguments>,
  Vec<String>,
>;
type RuntimeCustomArguments = (Vec<String>,);
type RuntimeCustomCallback =
  StrongMaybeAsyncJsCallback<RuntimeCustomArguments, FnArgs<RuntimeCustomArguments>, ()>;
type RuntimeNativeArguments = (serde_json::Value,);
type RuntimeNativeCallback =
  StrongMaybeAsyncJsCallback<RuntimeNativeArguments, FnArgs<RuntimeNativeArguments>, Vec<String>>;

/// Owns Hvigor command execution ordering after the live JavaScript task graph
/// has been projected into stable Rust task paths. JavaScript callbacks retain
/// only the official task/plugin object identities that cannot cross N-API.
struct RuntimeCommandExecutionCoordinator {
  plan: RuntimeCommandExecutionPlan,
  execute_standalone: RuntimeStandaloneCallback,
  execute_custom: RuntimeCustomCallback,
  execute_native: RuntimeNativeCallback,
}

impl RuntimeCommandExecutionCoordinator {
  fn from_js_object(plan: serde_json::Value, callbacks: Object<'_>) -> napi::Result<Self> {
    let plan = serde_json::from_value(plan).map_err(|error| {
      napi::Error::from_reason(format!("invalid runtime command execution plan: {error}"))
    })?;
    let execute_standalone = callbacks
      .get::<Function<RuntimeStandaloneArguments, MaybeAsyncJsReturn<Vec<String>>>>(
        "executeStandalone",
      )?
      .ok_or_else(|| {
        napi::Error::from_reason("runtime command callbacks are missing executeStandalone()")
      })?
      .build_threadsafe_function::<RuntimeStandaloneArguments>()
      .build_callback(
        |context: ThreadsafeCallContext<RuntimeStandaloneArguments>| {
          Ok(FnArgs {
            data: context.value,
          })
        },
      )?;
    let execute_custom = callbacks
      .get::<Function<RuntimeCustomArguments, MaybeAsyncJsReturn<()>>>("executeCustom")?
      .ok_or_else(|| {
        napi::Error::from_reason("runtime command callbacks are missing executeCustom()")
      })?
      .build_threadsafe_function::<RuntimeCustomArguments>()
      .build_callback(|context: ThreadsafeCallContext<RuntimeCustomArguments>| {
        Ok(FnArgs {
          data: context.value,
        })
      })?;
    let execute_native = callbacks
      .get::<Function<RuntimeNativeArguments, MaybeAsyncJsReturn<Vec<String>>>>("executeNative")?
      .ok_or_else(|| {
        napi::Error::from_reason("runtime command callbacks are missing executeNative()")
      })?
      .build_threadsafe_function::<RuntimeNativeArguments>()
      .build_callback(|context: ThreadsafeCallContext<RuntimeNativeArguments>| {
        Ok(FnArgs {
          data: context.value,
        })
      })?;
    Ok(Self {
      plan,
      execute_standalone: Arc::new(execute_standalone),
      execute_custom: Arc::new(execute_custom),
      execute_native: Arc::new(execute_native),
    })
  }

  async fn execute(self) -> anyhow::Result<RuntimeCommandExecutionReport> {
    let mut output = Vec::new();
    for task in self
      .plan
      .tasks
      .iter()
      .filter(|task| task.execution_lane == RuntimeCommandExecutionLane::Standalone)
    {
      output.extend(
        self
          .execute_standalone
          .await_call((task.task_path.clone(),))
          .await
          .map_err(|error| anyhow::anyhow!("execute standalone Hvigor task: {error}"))?,
      );
    }

    let sync_custom_paths = self.task_paths(RuntimeCommandExecutionLane::SyncCustom);
    self
      .execute_custom
      .await_call((sync_custom_paths,))
      .await
      .map_err(|error| anyhow::anyhow!("execute sync Hvigor custom tasks: {error}"))?;

    let custom_paths = self.task_paths(RuntimeCommandExecutionLane::Custom);
    if !self
      .plan
      .tasks
      .iter()
      .any(|task| task.execution_lane == RuntimeCommandExecutionLane::Native)
    {
      self
        .execute_custom
        .await_call((custom_paths,))
        .await
        .map_err(|error| anyhow::anyhow!("execute Hvigor custom tasks: {error}"))?;
      return Ok(RuntimeCommandExecutionReport {
        artifacts: Vec::new(),
        output,
      });
    }

    let mut artifacts = Vec::new();
    let mut seen_artifacts = FxHashSet::default();
    for request in self.plan.native_requests {
      for artifact in self
        .execute_native
        .await_call((request,))
        .await
        .map_err(|error| anyhow::anyhow!("execute native Hvigor request: {error}"))?
      {
        if seen_artifacts.insert(artifact.clone()) {
          artifacts.push(artifact);
        }
      }
    }
    Ok(RuntimeCommandExecutionReport { artifacts, output })
  }

  fn task_paths(&self, lane: RuntimeCommandExecutionLane) -> Vec<String> {
    self
      .plan
      .tasks
      .iter()
      .filter(|task| task.execution_lane == lane)
      .map(|task| task.task_path.clone())
      .collect()
  }
}

/// Execute a Rust-planned Hvigor command while keeping live task/plugin
/// objects behind three narrow JavaScript callbacks.
pub(crate) fn execute_runtime_command_plan<'env>(
  env: Env,
  plan: serde_json::Value,
  callbacks: Object<'env>,
) -> napi::Result<Object<'env>> {
  let coordinator = RuntimeCommandExecutionCoordinator::from_js_object(plan, callbacks)?;
  let raw_promise = napi::bindgen_prelude::execute_tokio_future(
    env.raw(),
    async move {
      let report = coordinator
        .execute()
        .await
        .map_err(|error| napi::Error::from_reason(format!("{error:#}")))?;
      serde_json::to_value(report).map_err(|error| {
        napi::Error::from_reason(format!(
          "serialize runtime command execution report: {error}"
        ))
      })
    },
    |env_raw, result| unsafe { napi::bindgen_prelude::ToNapiValue::to_napi_value(env_raw, result) },
  )?;
  unsafe { Object::from_napi_value(env.raw(), raw_promise) }
}
