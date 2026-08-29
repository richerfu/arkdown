use indexmap::IndexMap;
use napi::bindgen_prelude::{FromNapiValue, Function, JsObjectValue, Object};
use napi::threadsafe_function::ThreadsafeCallContext;
use napi::Env;
use napi_derive::napi;

use crate::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt, MaybeAsyncJsReturn};

type TaskHookCallback = MaybeAsyncJsCallback<(), (), ()>;

#[napi(object)]
pub struct RuntimeHvigorFileSetMutation {
  pub token: u32,
  pub replaced_token: Option<u32>,
}

#[napi(object)]
pub struct RuntimeHvigorFileSetEntry {
  pub path: String,
  pub token: u32,
}

/// Rust-owned ordered path inventory behind Hvigor's public `FileSet` class.
///
/// The associated options object remains in the owning V8 isolate because
/// `addEntry` exposes its exact object identity through `collect()`. Rust owns
/// insertion order, replacement, deletion, and live-entry selection; the
/// numeric token is only an ABI handle for that JavaScript value.
#[napi]
pub struct RuntimeHvigorFileSet {
  entries: IndexMap<String, u32>,
  next_token: u32,
}

#[napi]
impl RuntimeHvigorFileSet {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {
      entries: IndexMap::new(),
      next_token: 0,
    }
  }

  #[napi]
  pub fn add_entry(&mut self, path: String) -> RuntimeHvigorFileSetMutation {
    let token = self.allocate_token();
    let replaced_token = self.entries.insert(path, token);
    RuntimeHvigorFileSetMutation {
      token,
      replaced_token,
    }
  }

  #[napi]
  pub fn add_entries(&mut self, paths: Vec<String>) -> Vec<RuntimeHvigorFileSetMutation> {
    paths.into_iter().map(|path| self.add_entry(path)).collect()
  }

  #[napi]
  pub fn delete_entry(&mut self, path: String) -> Option<u32> {
    self.entries.shift_remove(&path)
  }

  #[napi]
  pub fn collect_entries(&self) -> Vec<RuntimeHvigorFileSetEntry> {
    self
      .entries
      .iter()
      .map(|(path, token)| RuntimeHvigorFileSetEntry {
        path: path.clone(),
        token: *token,
      })
      .collect()
  }

  #[napi(getter)]
  pub fn size(&self) -> u32 {
    self.entries.len() as u32
  }

  fn allocate_token(&mut self) -> u32 {
    let token = self.next_token;
    self.next_token = self.next_token.wrapping_add(1);
    token
  }
}

impl Default for RuntimeHvigorFileSet {
  fn default() -> Self {
    Self::new()
  }
}

#[napi(object)]
pub struct RuntimeHvigorTaskInfo {
  pub name: Option<String>,
  pub group: Option<String>,
  pub is_enabled: Option<bool>,
  pub description: Option<String>,
  pub source_plugin_id: Option<String>,
}

/// Primitive task metadata and dependency ownership for the public Hvigor
/// task hierarchy. Node objects and virtual task actions stay in JavaScript;
/// they are live V8 identities rather than build state.
#[napi]
pub struct RuntimeHvigorTaskState {
  name: Option<String>,
  group: Option<String>,
  enabled: Option<bool>,
  description: Option<String>,
  source_plugin_id: Option<String>,
  path: String,
  dependencies: Vec<String>,
}

#[napi]
impl RuntimeHvigorTaskState {
  #[napi(constructor)]
  pub fn new(node_name: String, project_task: bool, info: RuntimeHvigorTaskInfo) -> Self {
    let rendered_name = info.name.as_deref().unwrap_or("undefined");
    let path = if project_task {
      format!(":{rendered_name}")
    } else {
      format!("{node_name}:{rendered_name}")
    };
    Self {
      name: info.name,
      group: info.group,
      enabled: info.is_enabled,
      description: info.description,
      source_plugin_id: info.source_plugin_id,
      path,
      dependencies: Vec::new(),
    }
  }

  #[napi]
  pub fn get_name(&self) -> Option<String> {
    self.name.clone()
  }

  #[napi]
  pub fn get_path(&self) -> String {
    self.path.clone()
  }

  #[napi]
  pub fn set_depends_on(&mut self, dependencies: Vec<String>) {
    self.dependencies = dependencies;
  }

  #[napi]
  pub fn add_depends_on(&mut self, dependency: String) {
    self.dependencies.push(dependency);
  }

  #[napi]
  pub fn get_depends_on(&self) -> Vec<String> {
    self.dependencies.clone()
  }

  #[napi]
  pub fn get_enabled(&self) -> Option<bool> {
    self.enabled
  }

  #[napi]
  pub fn set_enabled(&mut self, enabled: Option<bool>) {
    self.enabled = enabled;
  }

  #[napi]
  pub fn set_description(&mut self, description: Option<String>) {
    self.description = description;
  }

  #[napi]
  pub fn get_description(&self) -> Option<String> {
    self.description.clone()
  }

  #[napi]
  pub fn get_group(&self) -> Option<String> {
    self.group.clone()
  }

  #[napi]
  pub fn set_group(&mut self, group: Option<String>) {
    self.group = group;
  }

  #[napi]
  pub fn get_source_plugin_id(&self) -> Option<String> {
    self.source_plugin_id.clone()
  }
}

/// Rust-owned task execution status used by worker and incremental execution.
#[napi]
pub struct RuntimeHvigorTaskExecutionState {
  worker_time_periods: Vec<Vec<f64>>,
  up_to_date: bool,
  state: u32,
  task_begin_time: Option<Vec<f64>>,
  untrack_state_reason: Option<String>,
  track_state: bool,
}

#[napi]
impl RuntimeHvigorTaskExecutionState {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {
      worker_time_periods: Vec::new(),
      up_to_date: false,
      state: 0,
      task_begin_time: None,
      untrack_state_reason: None,
      track_state: true,
    }
  }

  #[napi]
  pub fn set_state(&mut self, env: Env, state: u32) -> napi::Result<()> {
    if state < self.state {
      return Err(napi::Error::from_reason("Cannot change the task status."));
    }
    self.state = state;
    if state == 2 {
      self.task_begin_time = Some(self.hrtime(env)?);
    }
    Ok(())
  }

  #[napi]
  pub fn get_state(&self) -> u32 {
    self.state
  }

  #[napi]
  pub fn get_task_begin_time(&self) -> Option<Vec<f64>> {
    self.task_begin_time.clone()
  }

  #[napi]
  pub fn set_is_up_to_date(&mut self, value: bool) {
    self.up_to_date = value;
  }

  #[napi]
  pub fn is_up_to_date(&self) -> bool {
    self.up_to_date
  }

  #[napi]
  pub fn set_worker_time_period(&mut self, value: Vec<f64>) {
    self.worker_time_periods.push(value);
  }

  #[napi]
  pub fn get_worker_time_period(&self) -> Vec<Vec<f64>> {
    self.worker_time_periods.clone()
  }

  #[napi(getter)]
  pub fn un_track_state_reason(&self) -> Option<String> {
    self.untrack_state_reason.clone()
  }

  #[napi(setter)]
  pub fn set_un_track_state_reason(&mut self, value: Option<String>) {
    self.untrack_state_reason = value;
    self.track_state = false;
  }

  #[napi]
  pub fn is_track_state(&self) -> bool {
    self.track_state
  }

  fn hrtime(&self, env: Env) -> napi::Result<Vec<f64>> {
    let global = env.get_global()?;
    let process: Object<'_> = global.get_named_property("process")?;
    let hrtime: Function<'_, (), Vec<f64>> = process.get_named_property("hrtime")?;
    hrtime.apply(process, ())
  }
}

impl Default for RuntimeHvigorTaskExecutionState {
  fn default() -> Self {
    Self::new()
  }
}

/// Owns latest-hvigor's LIFO `beforeRun` and FIFO `afterRun` scheduling.
/// Callback bodies remain in V8, while ordering, draining, and sequential
/// Promise awaiting are performed by Rust.
#[napi]
pub struct RuntimeHvigorTaskHooks {
  before: Vec<TaskHookCallback>,
  after: Vec<TaskHookCallback>,
}

#[napi]
impl RuntimeHvigorTaskHooks {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {
      before: Vec::new(),
      after: Vec::new(),
    }
  }

  #[napi]
  pub fn before_run(
    &mut self,
    #[napi(ts_arg_type = "() => void | Promise<void>")] callback: Function<
      (),
      MaybeAsyncJsReturn<()>,
    >,
  ) -> napi::Result<()> {
    self.before.push(self.callback(callback)?);
    Ok(())
  }

  #[napi]
  pub fn after_run(
    &mut self,
    #[napi(ts_arg_type = "() => void | Promise<void>")] callback: Function<
      (),
      MaybeAsyncJsReturn<()>,
    >,
  ) -> napi::Result<()> {
    self.after.push(self.callback(callback)?);
    Ok(())
  }

  #[napi(getter)]
  pub fn before_count(&self) -> u32 {
    self.before.len() as u32
  }

  #[napi(getter)]
  pub fn after_count(&self) -> u32 {
    self.after.len() as u32
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn execute_before<'env>(&mut self, env: Env) -> napi::Result<Object<'env>> {
    let callbacks = std::mem::take(&mut self.before).into_iter().rev().collect();
    self.execute(env, callbacks, "beforeRun")
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn execute_after<'env>(&mut self, env: Env) -> napi::Result<Object<'env>> {
    let callbacks = std::mem::take(&mut self.after);
    self.execute(env, callbacks, "afterRun")
  }

  fn callback(
    &self,
    callback: Function<(), MaybeAsyncJsReturn<()>>,
  ) -> napi::Result<TaskHookCallback> {
    let callback = callback
      .build_threadsafe_function::<()>()
      .weak::<true>()
      .build_callback(|_context: ThreadsafeCallContext<()>| Ok(()))?;
    Ok(std::sync::Arc::new(callback))
  }

  fn execute<'env>(
    &self,
    env: Env,
    callbacks: Vec<TaskHookCallback>,
    phase: &'static str,
  ) -> napi::Result<Object<'env>> {
    let raw_promise = napi::bindgen_prelude::execute_tokio_future(
      env.raw(),
      async move {
        for callback in callbacks {
          callback.await_call(()).await.map_err(|error| {
            napi::Error::from_reason(format!("Hvigor task {phase} callback failed: {error}"))
          })?;
        }
        Ok::<(), napi::Error>(())
      },
      |env_raw, result| unsafe {
        napi::bindgen_prelude::ToNapiValue::to_napi_value(env_raw, result)
      },
    )?;
    unsafe { Object::from_napi_value(env.raw(), raw_promise) }
  }
}

impl Default for RuntimeHvigorTaskHooks {
  fn default() -> Self {
    Self::new()
  }
}
