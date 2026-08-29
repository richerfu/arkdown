use std::collections::BTreeMap;
use std::sync::Arc;

use napi::bindgen_prelude::{FnArgs, FromNapiValue, Function, Object};
use napi::threadsafe_function::ThreadsafeCallContext;
use napi::Env;
use napi_derive::napi;

use crate::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt, MaybeAsyncJsReturn};

type LifecycleArguments = (u32,);
type LifecycleCallback = MaybeAsyncJsCallback<LifecycleArguments, FnArgs<LifecycleArguments>, ()>;

const LIFECYCLE_HOOKS: [&str; 8] = [
  "configEvaluated",
  "nodesInitialized",
  "beforeNodeEvaluate",
  "afterNodeEvaluate",
  "nodesEvaluated",
  "taskGraphResolved",
  "buildFinished",
  "onWatchWorkerMessage",
];

/// Owns latest-hvigor lifecycle registration order and sequential callback
/// scheduling. The numeric invocation token resolves the live hook argument
/// in JavaScript because arbitrary objects cannot leave their V8 isolate.
#[napi]
pub struct RuntimeHvigorLifecycle {
  callbacks: BTreeMap<String, Vec<LifecycleCallback>>,
}

#[napi]
impl RuntimeHvigorLifecycle {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {
      callbacks: LIFECYCLE_HOOKS
        .into_iter()
        .map(|name| (name.to_string(), Vec::new()))
        .collect(),
    }
  }

  #[napi]
  pub fn register(
    &mut self,
    name: String,
    #[napi(ts_arg_type = "(invocation: number) => void | Promise<void>")] callback: Function<
      LifecycleArguments,
      MaybeAsyncJsReturn<()>,
    >,
  ) -> napi::Result<()> {
    let callbacks = self
      .callbacks
      .get_mut(&name)
      .ok_or_else(|| napi::Error::from_reason(format!("unknown Hvigor lifecycle hook '{name}'")))?;
    let callback = callback
      .build_threadsafe_function::<LifecycleArguments>()
      .weak::<true>()
      .build_callback(|context: ThreadsafeCallContext<LifecycleArguments>| {
        Ok(FnArgs {
          data: context.value,
        })
      })?;
    callbacks.push(Arc::new(callback));
    Ok(())
  }

  #[napi]
  pub fn clear(&mut self) {
    for callbacks in self.callbacks.values_mut() {
      callbacks.clear();
    }
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn run_hook<'env>(
    &self,
    env: Env,
    name: String,
    invocation: u32,
  ) -> napi::Result<Object<'env>> {
    let callbacks =
      self.callbacks.get(&name).cloned().ok_or_else(|| {
        napi::Error::from_reason(format!("unknown Hvigor lifecycle hook '{name}'"))
      })?;
    let hook_name = name.clone();
    let raw_promise = napi::bindgen_prelude::execute_tokio_future(
      env.raw(),
      async move {
        for callback in callbacks {
          callback.await_call((invocation,)).await.map_err(|error| {
            napi::Error::from_reason(format!(
              "Hvigor lifecycle hook '{hook_name}' failed: {error}"
            ))
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

impl Default for RuntimeHvigorLifecycle {
  fn default() -> Self {
    Self::new()
  }
}
