use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use napi::bindgen_prelude::{FnArgs, Function, Object};
use napi::threadsafe_function::ThreadsafeCallContext;
use napi_derive::napi;
use rustc_hash::FxHashSet;
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, oneshot};

use crate::cmd::build::coordinator::{
  BuildAbcTransformEvent, BuildCycleEvent, BuildNativeCommandEvent, BuildTaskEvent,
  BuildTaskObserver,
};
use crate::js_callback::{MaybeAsyncJsCallbackExt, MaybeAsyncJsReturn, StrongMaybeAsyncJsCallback};

use super::catalog::RuntimeTaskCatalogRequest;
use super::engine::{RuntimeTaskCallback, RuntimeTaskEngine};
use super::model::{
  RuntimeTaskCallbackResult, RuntimeTaskDescriptor, RuntimeTaskModelBuilder,
  RuntimeTaskOperationPhase, RuntimeTaskSelection,
};

type RuntimeTaskDispatchArguments = (serde_json::Value,);
type RuntimeTaskDispatchTsfn = StrongMaybeAsyncJsCallback<
  RuntimeTaskDispatchArguments,
  FnArgs<RuntimeTaskDispatchArguments>,
  JsRuntimeTaskCallbackResult,
>;
type NativeEventArguments = (serde_json::Value,);
type TransformAbcTsfn =
  StrongMaybeAsyncJsCallback<NativeEventArguments, FnArgs<NativeEventArguments>, ()>;
type ModifyNativeCommandTsfn =
  StrongMaybeAsyncJsCallback<NativeEventArguments, FnArgs<NativeEventArguments>, Vec<String>>;

#[napi(object)]
pub struct JsRuntimeTaskCallbackResult {
  pub enabled: Option<bool>,
  pub track_state: Option<bool>,
  pub refresh_declarations: Option<bool>,
  pub has_before_run: Option<bool>,
  pub has_after_run: Option<bool>,
  pub has_failed: Option<bool>,
  pub has_task_should_do: Option<bool>,
  pub task_should_do: Option<bool>,
  #[napi(ts_type = "Array<Record<string, unknown>> | undefined")]
  pub input_properties: Option<serde_json::Value>,
  #[napi(ts_type = "Array<string | Record<string, unknown>> | undefined")]
  pub input_files: Option<serde_json::Value>,
  #[napi(ts_type = "Array<string | Record<string, unknown>> | undefined")]
  pub output_files: Option<serde_json::Value>,
  pub output: Option<Vec<String>>,
}

impl JsRuntimeTaskCallbackResult {
  fn into_runtime_result(self) -> anyhow::Result<RuntimeTaskCallbackResult> {
    Ok(RuntimeTaskCallbackResult {
      enabled: self.enabled,
      track_state: self.track_state,
      refresh_declarations: self.refresh_declarations,
      has_before_run: self.has_before_run,
      has_after_run: self.has_after_run,
      has_failed: self.has_failed,
      has_task_should_do: self.has_task_should_do,
      task_should_do: self.task_should_do,
      input_properties: self
        .input_properties
        .map(serde_json::from_value)
        .transpose()?,
      input_files: self.input_files.map(serde_json::from_value).transpose()?,
      output_files: self.output_files.map(serde_json::from_value).transpose()?,
      output: self.output.unwrap_or_default(),
    })
  }
}

pub struct RuntimeTaskBridgeSeed {
  tasks: Vec<RuntimeTaskDescriptor>,
  selection: RuntimeTaskSelection,
  dispatch_task: RuntimeTaskDispatchTsfn,
  transform_abc: TransformAbcTsfn,
  modify_native_command: ModifyNativeCommandTsfn,
  command_modifier_types: FxHashSet<u8>,
  has_abc_transform_consumers: bool,
}

impl RuntimeTaskBridgeSeed {
  pub fn from_js_object(object: Object<'_>) -> napi::Result<Self> {
    let overlays: Vec<RuntimeTaskDescriptor> = Self::required_property(&object, "tasks")?;
    let catalogs: Vec<RuntimeTaskCatalogRequest> = Self::default_property(&object, "catalogs")?;
    let removed_task_paths: Vec<String> = Self::default_property(&object, "removedTaskPaths")?;
    let selection: RuntimeTaskSelection = Self::required_property(&object, "selection")?;
    let dispatch_task = Self::dispatch_task_callback(&object)?;
    let transform_abc = Self::transform_abc_callback(&object)?;
    let modify_native_command = Self::modify_native_command_callback(&object)?;
    let command_modifier_types = object
      .get::<Vec<u32>>("commandModifierTypes")?
      .unwrap_or_default()
      .into_iter()
      .filter_map(|value| u8::try_from(value).ok())
      .collect();
    let has_abc_transform_consumers = object
      .get::<bool>("hasAbcTransformConsumers")?
      .unwrap_or(false);
    let mut tasks = RuntimeTaskModelBuilder::new(catalogs, overlays, removed_task_paths)
      .build()
      .map_err(|error| napi::Error::from_reason(format!("invalid runtime task model: {error}")))?;
    selection.normalize_declared_paths(&mut tasks);
    Ok(Self {
      tasks,
      selection,
      dispatch_task,
      transform_abc,
      modify_native_command,
      command_modifier_types,
      has_abc_transform_consumers,
    })
  }

  /// Whether this build actually needs the custom-task engine.
  ///
  /// The engine exists only to dispatch hvigorfile.ts custom tasks and plugin
  /// callbacks at native lifecycle boundaries (CLAUDE.md rule 12 narrow
  /// exemption). When every declared task is built-in, no native command
  /// builder is active, and no `transformAbc` consumer is registered, the
  /// observer is dead weight — every task boundary would cross the mpsc actor
  /// channel only to look up graph membership and do nothing.
  pub fn has_runtime_work(&self) -> bool {
    needs_runtime_work(
      &self.tasks,
      &self.command_modifier_types,
      self.has_abc_transform_consumers,
    )
  }

  fn required_property<T: DeserializeOwned>(object: &Object<'_>, key: &str) -> napi::Result<T> {
    let value = object
      .get::<serde_json::Value>(key)?
      .ok_or_else(|| napi::Error::from_reason(format!("runtime task bridge is missing {key}")))?;
    serde_json::from_value(value).map_err(|error| {
      napi::Error::from_reason(format!("invalid runtime task bridge {key}: {error}"))
    })
  }

  fn default_property<T>(object: &Object<'_>, key: &str) -> napi::Result<T>
  where
    T: Default + DeserializeOwned,
  {
    object
      .get::<serde_json::Value>(key)?
      .map(serde_json::from_value)
      .transpose()
      .map(Option::unwrap_or_default)
      .map_err(|error| {
        napi::Error::from_reason(format!("invalid runtime task bridge {key}: {error}"))
      })
  }

  fn dispatch_task_callback(object: &Object<'_>) -> napi::Result<RuntimeTaskDispatchTsfn> {
    let callback = object
      .get::<
        Function<
          RuntimeTaskDispatchArguments,
          MaybeAsyncJsReturn<JsRuntimeTaskCallbackResult>,
        >,
      >("dispatchTask")?
      .ok_or_else(|| napi::Error::from_reason("runtime task bridge is missing dispatchTask()"))?
      .build_threadsafe_function::<RuntimeTaskDispatchArguments>()
      .build_callback(
        |context: ThreadsafeCallContext<RuntimeTaskDispatchArguments>| {
          Ok(FnArgs {
            data: context.value,
          })
        },
      )?;
    Ok(Arc::new(callback))
  }

  fn transform_abc_callback(object: &Object<'_>) -> napi::Result<TransformAbcTsfn> {
    let callback = object
      .get::<Function<NativeEventArguments, MaybeAsyncJsReturn<()>>>("transformAbc")?
      .ok_or_else(|| napi::Error::from_reason("runtime task bridge is missing transformAbc()"))?
      .build_threadsafe_function::<NativeEventArguments>()
      .build_callback(|context: ThreadsafeCallContext<NativeEventArguments>| {
        Ok(FnArgs {
          data: context.value,
        })
      })?;
    Ok(Arc::new(callback))
  }

  fn modify_native_command_callback(object: &Object<'_>) -> napi::Result<ModifyNativeCommandTsfn> {
    let callback = object
      .get::<Function<NativeEventArguments, MaybeAsyncJsReturn<Vec<String>>>>(
        "modifyNativeCommand",
      )?
      .ok_or_else(|| {
        napi::Error::from_reason("runtime task bridge is missing modifyNativeCommand()")
      })?
      .build_threadsafe_function::<NativeEventArguments>()
      .build_callback(|context: ThreadsafeCallContext<NativeEventArguments>| {
        Ok(FnArgs {
          data: context.value,
        })
      })?;
    Ok(Arc::new(callback))
  }

  pub fn into_engine(self) -> anyhow::Result<RuntimeTaskEngine> {
    RuntimeTaskEngine::new(
      self.tasks,
      self.selection,
      Box::new(NapiRuntimeTaskCallback {
        dispatch_task: self.dispatch_task,
        transform_abc: self.transform_abc,
        modify_native_command: self.modify_native_command,
      }),
    )
    .map_err(Into::into)
  }

  pub fn declared_input_paths_json(&self) -> anyhow::Result<String> {
    serde_json::to_string(&self.selection.declared_input_paths(&self.tasks)).map_err(Into::into)
  }

  pub fn project_cache_dir(&self) -> PathBuf {
    self.selection.project_root.join(".hvigor")
  }

  pub async fn execute(self) -> anyhow::Result<Vec<String>> {
    let mut engine = self.into_engine()?;
    engine.execute_command_roots().await.map_err(Into::into)
  }
}

struct NapiRuntimeTaskCallback {
  dispatch_task: RuntimeTaskDispatchTsfn,
  transform_abc: TransformAbcTsfn,
  modify_native_command: ModifyNativeCommandTsfn,
}

#[async_trait]
impl RuntimeTaskCallback for NapiRuntimeTaskCallback {
  async fn invoke_task(
    &mut self,
    task_path: &str,
    phase: RuntimeTaskOperationPhase,
    error: Option<String>,
  ) -> anyhow::Result<RuntimeTaskCallbackResult> {
    let output = self
      .dispatch_task
      .await_call((serde_json::json!({
        "taskPath": task_path,
        "phase": RuntimeTaskEngine::phase_name(phase),
        "error": error,
      }),))
      .await
      .map_err(|error| anyhow::anyhow!("runtime task JS dispatcher failed: {error}"))?;
    output.into_runtime_result()
  }

  async fn transform_abc(&mut self, event: serde_json::Value) -> anyhow::Result<()> {
    self
      .transform_abc
      .await_call((event,))
      .await
      .map_err(|error| anyhow::anyhow!("runtime transformAbc callback failed: {error}"))
  }

  async fn modify_native_command(
    &mut self,
    event: serde_json::Value,
  ) -> anyhow::Result<Vec<String>> {
    self
      .modify_native_command
      .await_call((event,))
      .await
      .map_err(|error| anyhow::anyhow!("runtime native command callback failed: {error}"))
  }
}

/// Pure decision input of [`RuntimeTaskBridgeSeed::has_runtime_work`]: the
/// custom-task engine is needed only when a non-built-in task exists, a native
/// command builder is active, or a `transformAbc` consumer is registered.
pub(crate) fn needs_runtime_work(
  tasks: &[RuntimeTaskDescriptor],
  command_modifier_types: &FxHashSet<u8>,
  has_abc_transform_consumers: bool,
) -> bool {
  tasks.iter().any(|task| !task.built_in)
    || !command_modifier_types.is_empty()
    || has_abc_transform_consumers
}

enum RuntimeTaskCommand {
  Begin(oneshot::Sender<anyhow::Result<bool>>),
  Finish(oneshot::Sender<anyhow::Result<bool>>),
  Before(BuildTaskEvent, oneshot::Sender<anyhow::Result<bool>>),
  After(BuildTaskEvent, oneshot::Sender<anyhow::Result<bool>>),
  UpToDate(BuildTaskEvent, oneshot::Sender<anyhow::Result<bool>>),
  TransformAbc(
    BuildAbcTransformEvent,
    oneshot::Sender<anyhow::Result<bool>>,
  ),
  ModifyNativeCommand(
    BuildNativeCommandEvent,
    oneshot::Sender<anyhow::Result<Vec<String>>>,
  ),
}

pub struct RuntimeTaskActorObserver {
  sender: mpsc::Sender<RuntimeTaskCommand>,
  command_modifier_types: FxHashSet<u8>,
}

impl RuntimeTaskActorObserver {
  /// Spawn the observer actor, or `None` when the build registers no custom
  /// tasks, no active command modifiers, and no `transformAbc` consumers.
  /// Returning `None` keeps the coordinator on its observer-free fast path
  /// (see CLAUDE.md rule 12 narrow exemption).
  pub fn spawn(
    seed: RuntimeTaskBridgeSeed,
  ) -> anyhow::Result<Option<Arc<dyn BuildTaskObserver>>> {
    if !seed.has_runtime_work() {
      return Ok(None);
    }
    let command_modifier_types = seed.command_modifier_types.clone();
    let engine = seed.into_engine()?;
    let (sender, receiver) = mpsc::channel(8);
    tokio::spawn(RuntimeTaskActor { engine, receiver }.run());
    Ok(Some(Arc::new(Self {
      sender,
      command_modifier_types,
    })))
  }

  async fn request(
    &self,
    command: impl FnOnce(oneshot::Sender<anyhow::Result<bool>>) -> RuntimeTaskCommand,
  ) -> anyhow::Result<bool> {
    let (sender, receiver) = oneshot::channel();
    self.sender.send(command(sender)).await.map_err(|_| {
      anyhow::anyhow!("runtime task actor stopped before receiving the build event")
    })?;
    receiver.await.map_err(|_| {
      anyhow::anyhow!("runtime task actor stopped before replying to the build event")
    })?
  }

  async fn request_command(&self, event: BuildNativeCommandEvent) -> anyhow::Result<Vec<String>> {
    let (sender, receiver) = oneshot::channel();
    self
      .sender
      .send(RuntimeTaskCommand::ModifyNativeCommand(event, sender))
      .await
      .map_err(|_| {
        anyhow::anyhow!("runtime task actor stopped before receiving the native command event")
      })?;
    receiver.await.map_err(|_| {
      anyhow::anyhow!("runtime task actor stopped before replying to the native command event")
    })?
  }
}

#[async_trait]
impl BuildTaskObserver for RuntimeTaskActorObserver {
  async fn build_cycle_started(&self, _event: BuildCycleEvent) -> anyhow::Result<()> {
    self.request(RuntimeTaskCommand::Begin).await.map(|_| ())
  }

  async fn build_cycle_finished(&self, _event: BuildCycleEvent) -> anyhow::Result<()> {
    self.request(RuntimeTaskCommand::Finish).await.map(|_| ())
  }

  async fn before_task(&self, event: BuildTaskEvent) -> anyhow::Result<bool> {
    self
      .request(|reply| RuntimeTaskCommand::Before(event, reply))
      .await
  }

  async fn after_task(&self, event: BuildTaskEvent) -> anyhow::Result<()> {
    self
      .request(|reply| RuntimeTaskCommand::After(event, reply))
      .await
      .map(|_| ())
  }

  async fn task_up_to_date(&self, event: BuildTaskEvent) -> anyhow::Result<()> {
    self
      .request(|reply| RuntimeTaskCommand::UpToDate(event, reply))
      .await
      .map(|_| ())
  }

  async fn transform_abc(&self, event: BuildAbcTransformEvent) -> anyhow::Result<()> {
    self
      .request(|reply| RuntimeTaskCommand::TransformAbc(event, reply))
      .await
      .map(|_| ())
  }

  fn has_native_command_modifier(&self, builder_type: u8) -> bool {
    self.command_modifier_types.contains(&builder_type)
  }

  async fn modify_native_command(
    &self,
    event: BuildNativeCommandEvent,
  ) -> anyhow::Result<Vec<String>> {
    self.request_command(event).await
  }
}

struct RuntimeTaskActor {
  engine: RuntimeTaskEngine,
  receiver: mpsc::Receiver<RuntimeTaskCommand>,
}

impl RuntimeTaskActor {
  async fn run(mut self) {
    while let Some(command) = self.receiver.recv().await {
      match command {
        RuntimeTaskCommand::Begin(reply) => {
          self.engine.begin_cycle();
          let _ = reply.send(Ok(true));
        }
        RuntimeTaskCommand::Finish(reply) => {
          let _ = reply.send(
            self
              .engine
              .finish_cycle()
              .await
              .map(|_| ())
              .map(|_| true)
              .map_err(Into::into),
          );
        }
        RuntimeTaskCommand::Before(event, reply) => {
          let _ = reply.send(
            self
              .engine
              .before_native_task(
                &event.module_name,
                event.target_name.as_deref(),
                &event.task_name,
              )
              .await
              .map_err(Into::into),
          );
        }
        RuntimeTaskCommand::After(event, reply) => {
          let _ = reply.send(
            self
              .engine
              .after_native_task(
                &event.module_name,
                event.target_name.as_deref(),
                &event.task_name,
              )
              .await
              .map(|_| true)
              .map_err(Into::into),
          );
        }
        RuntimeTaskCommand::UpToDate(event, reply) => {
          let _ = reply.send(
            self
              .engine
              .native_task_up_to_date(
                &event.module_name,
                event.target_name.as_deref(),
                &event.task_name,
              )
              .await
              .map(|_| true)
              .map_err(Into::into),
          );
        }
        RuntimeTaskCommand::TransformAbc(event, reply) => {
          let result = match serde_json::to_value(event) {
            Ok(event) => self
              .engine
              .transform_abc(event)
              .await
              .map(|_| true)
              .map_err(Into::into),
            Err(error) => Err(error.into()),
          };
          let _ = reply.send(result);
        }
        RuntimeTaskCommand::ModifyNativeCommand(event, reply) => {
          let result = match serde_json::to_value(event) {
            Ok(event) => self
              .engine
              .modify_native_command(event)
              .await
              .map_err(Into::into),
            Err(error) => Err(error.into()),
          };
          let _ = reply.send(result);
        }
      }
    }
  }
}
