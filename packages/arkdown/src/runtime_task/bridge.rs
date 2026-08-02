use std::sync::Arc;

use async_trait::async_trait;
use napi::bindgen_prelude::{Either, Function, Object, Promise};
use napi::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction};
use tokio::sync::{mpsc, oneshot};

use crate::cmd::build::coordinator::{
  BuildAbcTransformEvent, BuildCycleEvent, BuildTaskEvent, BuildTaskObserver,
};

use super::engine::{RuntimeTaskCallback, RuntimeTaskEngine};
use super::model::{
  RuntimeTaskCallbackResult, RuntimeTaskDescriptor, RuntimeTaskOperation, RuntimeTaskSelection,
};

type StringOrPromise = Either<String, Promise<String>>;
type RuntimeTaskTsfn = Arc<
  ThreadsafeFunction<
    String,
    StringOrPromise,
    napi::bindgen_prelude::FnArgs<(String,)>,
    napi::Status,
    false,
  >,
>;

pub struct RuntimeTaskBridgeSeed {
  tasks: Vec<RuntimeTaskDescriptor>,
  selection: RuntimeTaskSelection,
  callback: RuntimeTaskTsfn,
}

impl RuntimeTaskBridgeSeed {
  pub fn from_js_object(object: Object<'_>) -> napi::Result<Self> {
    let tasks_json = object
      .get::<String>("tasksJson")?
      .ok_or_else(|| napi::Error::from_reason("runtime task bridge is missing tasksJson"))?;
    let selection_json = object
      .get::<String>("selectionJson")?
      .ok_or_else(|| napi::Error::from_reason("runtime task bridge is missing selectionJson"))?;
    let callback = object
      .get::<Function<(String,), StringOrPromise>>("invoke")?
      .ok_or_else(|| napi::Error::from_reason("runtime task bridge is missing invoke()"))?
      .build_threadsafe_function::<String>()
      .build_callback(|context: ThreadsafeCallContext<String>| {
        Ok(napi::bindgen_prelude::FnArgs {
          data: (context.value,),
        })
      })?;
    let mut tasks =
      serde_json::from_str::<Vec<RuntimeTaskDescriptor>>(&tasks_json).map_err(|error| {
        napi::Error::from_reason(format!("invalid runtime task descriptors: {error}"))
      })?;
    let selection =
      serde_json::from_str::<RuntimeTaskSelection>(&selection_json).map_err(|error| {
        napi::Error::from_reason(format!("invalid runtime task selection: {error}"))
      })?;
    selection.normalize_declared_paths(&mut tasks);
    Ok(Self {
      tasks,
      selection,
      callback: Arc::new(callback),
    })
  }

  pub fn into_engine(self) -> anyhow::Result<RuntimeTaskEngine> {
    RuntimeTaskEngine::new(
      self.tasks,
      self.selection,
      Box::new(NapiRuntimeTaskCallback {
        callback: self.callback,
      }),
    )
    .map_err(Into::into)
  }

  pub fn declared_input_paths_json(&self) -> anyhow::Result<String> {
    serde_json::to_string(&self.selection.declared_input_paths(&self.tasks)).map_err(Into::into)
  }

  pub async fn execute(self) -> anyhow::Result<Vec<String>> {
    let mut engine = self.into_engine()?;
    engine.execute_command_roots().await.map_err(Into::into)
  }
}

struct NapiRuntimeTaskCallback {
  callback: RuntimeTaskTsfn,
}

#[async_trait]
impl RuntimeTaskCallback for NapiRuntimeTaskCallback {
  async fn invoke(
    &mut self,
    operation: RuntimeTaskOperation,
  ) -> anyhow::Result<RuntimeTaskCallbackResult> {
    let operation_json = serde_json::to_string(&operation)?;
    let returned = self
      .callback
      .call_async_catch(operation_json)
      .await
      .map_err(|error| anyhow::anyhow!("runtime task JS callback failed: {error}"))?;
    let output = match returned {
      Either::A(output) => output,
      Either::B(promise) => promise
        .await
        .map_err(|error| anyhow::anyhow!("runtime task JS callback promise failed: {error}"))?,
    };
    if output.is_empty() {
      return Ok(RuntimeTaskCallbackResult::default());
    }
    serde_json::from_str(&output)
      .map_err(|error| anyhow::anyhow!("invalid runtime task JS callback result: {error}"))
  }
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
}

pub struct RuntimeTaskActorObserver {
  sender: mpsc::Sender<RuntimeTaskCommand>,
}

impl RuntimeTaskActorObserver {
  pub fn spawn(seed: RuntimeTaskBridgeSeed) -> anyhow::Result<Arc<dyn BuildTaskObserver>> {
    let engine = seed.into_engine()?;
    let (sender, receiver) = mpsc::channel(8);
    tokio::spawn(RuntimeTaskActor { engine, receiver }.run());
    Ok(Arc::new(Self { sender }))
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
          let result = match serde_json::to_string(&event) {
            Ok(event_json) => self
              .engine
              .transform_abc(event_json)
              .await
              .map(|_| true)
              .map_err(Into::into),
            Err(error) => Err(error.into()),
          };
          let _ = reply.send(result);
        }
      }
    }
  }
}
