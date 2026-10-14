//! arkdown Node.js bindings (napi-rs 3.9.x).
//!
//! Exposes typed build, check, and clean services. `build()` accepts
//! Rolldown-compatible JS plugin objects; plugin hooks are called via NAPI
//! `ThreadsafeFunction` in the same process and V8 heap.

#![deny(clippy::all)]

// Debug mimalloc builds retain internal arena assertions and can abort while
// Node and Rayon tear their worker threads down concurrently. Keep the
// production allocator win, but use the platform allocator for debug NAPI
// builds so a successful hvigor-compatible command exits reliably.
#[cfg(all(
  feature = "allocator",
  not(debug_assertions),
  not(feature = "dhat-heap"),
  not(any(
    target_arch = "arm",
    target_os = "android",
    target_os = "freebsd",
    target_os = "windows",
    target_family = "wasm"
  ))
))]
#[global_allocator]
static ALLOC: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

/// Use the same bounded N-API runtime contract as Rolldown's native binding.
/// Rolldown performs CPU work on Tokio workers and uses `spawn_blocking` for a
/// small amount of source I/O; Tokio's default allowance of 512 blocking
/// threads otherwise lets a large cold graph create hundreds of parked stacks.
#[napi_derive::module_init]
fn initialize_async_runtime() {
  #[cfg(all(
    feature = "allocator",
    not(debug_assertions),
    not(feature = "dhat-heap"),
    not(any(
      target_arch = "arm",
      target_os = "android",
      target_os = "freebsd",
      target_os = "windows",
      target_family = "wasm"
    ))
  ))]
  {
    // mimalloc-safe exposes no option setter. libmimalloc-sys2 0.1.60 exposes
    // mi_option_set but omits this constant: mimalloc.h assigns purge_delay 15.
    const MI_OPTION_PURGE_DELAY: libmimalloc_sys2::mi_option_t = 15;
    // Round 1 starts at 0 ms; the final delay awaits acceptance measurements.
    // SAFETY: set the valid option once at binding initialization, before
    // starting the runtime or exposing any build/check services to JavaScript.
    unsafe { libmimalloc_sys2::mi_option_set(MI_OPTION_PURGE_DELAY, 0) };
  }
  #[cfg(not(target_family = "wasm"))]
  {
    let max_blocking_threads = std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS")
      .ok()
      .and_then(|value| value.parse::<usize>().ok())
      .unwrap_or(4);
    let worker_threads = std::env::var("ROLLDOWN_WORKER_THREADS")
      .ok()
      .and_then(|value| value.parse::<usize>().ok())
      .unwrap_or_else(|| num_cpus::get_physical() * 3 / 2);
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    heap_reclaimer::NapiHeapReclaimer::configure_runtime(&mut builder, worker_threads);
    let runtime = builder
      .worker_threads(worker_threads)
      .max_blocking_threads(max_blocking_threads)
      .enable_all()
      .build()
      .expect("failed to initialize the Arkdown N-API runtime");
    napi::bindgen_prelude::create_custom_tokio_runtime(runtime);
  }
}

mod heap_reclaimer;
mod hvigor_plugin_plan;
mod js_callback;
mod plugin_module;
mod runtime_observe;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{
  ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi_derive::napi;
use rustc_hash::FxHashMap;

use js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt, MaybeAsyncJsReturn};

use rolldown_common::{
  side_effects::HookSideEffects, EmittedAsset, EmittedChunk, LogWithoutPlugin, ModuleDefFormat,
  ModuleInfo, ModuleType, NormalModule, PreserveEntrySignatures, ResolvedExternal, StrOrBytes,
};
use rolldown_plugin::{
  typedmap::TypedMapKey, CustomField, HookBuildEndArgs, HookBuildStartArgs, HookLoadArgs,
  HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn,
  HookTransformArgs, HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage,
  Plugin, PluginContext, PluginContextResolveOptions, PluginHookMeta,
  PluginOrder as RolldownPluginOrder, SharedLoadPluginContext, SharedTransformPluginContext,
};

use arkdown::plugins::factory::{
  HvigorHookFuture, HvigorHookOrder, HvigorLifecycleHook, HvigorPluginGapHooks, HvigorPluginOrder,
  PackagePlugin,
};
use arkdown::plugins::hvigor_compile::{
  HvigorCompilePluginImportEntry, HvigorCompilePluginPlanConfig, HvigorCompilePluginShare,
  HvigorCompilePluginSourceFile, HvigorNativePluginSlot, HvigorWatchChange,
};
use arkdown::PluginCacheModule;
use arkdown_ets_checker::{
  NativeCheckError, NativeOhSdkPluginCompletion, NativeOhSdkPluginHost, NativeOhSdkPluginRequest,
  NativeOhSdkPluginResponse,
};

#[napi(object)]
pub struct CleanConfig {
  pub project: Option<String>,
}

#[napi(object)]
pub struct CheckConfig {
  pub project: Option<String>,
  pub target: Option<String>,
  pub module: Option<String>,
  pub is_crossplatform: Option<bool>,
  pub ignore_crossplatform_check: Option<bool>,
}

type SdkPluginDispatchTsfn =
  Arc<ThreadsafeFunction<String, String, FnArgs<(String,)>, Status, false, false>>;

struct NapiSdkPluginHost {
  dispatch: SdkPluginDispatchTsfn,
}

impl std::fmt::Debug for NapiSdkPluginHost {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("NapiSdkPluginHost")
  }
}

impl NapiSdkPluginHost {
  fn from_dispatch(
    dispatch: Function<'_, (String,), String>,
  ) -> napi::Result<Arc<dyn NativeOhSdkPluginHost>> {
    let dispatch = Arc::new(
      dispatch
        .build_threadsafe_function::<String>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?,
    );
    Ok(Arc::new(Self { dispatch }))
  }
}

impl NativeOhSdkPluginHost for NapiSdkPluginHost {
  fn invoke_batch(
    &self,
    requests: Vec<NativeOhSdkPluginRequest>,
  ) -> std::result::Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    self.submit_batch(
      requests,
      Box::new(move |result| {
        let _ = sender.send(result);
      }),
    );
    receiver.recv().map_err(|error| {
      NativeCheckError::Compiler(format!(
        "OH SDK plugin callback batch did not respond: {error}"
      ))
    })?
  }

  fn submit_batch(
    &self,
    requests: Vec<NativeOhSdkPluginRequest>,
    complete: NativeOhSdkPluginCompletion,
  ) {
    let request = match serde_json::to_string(&requests) {
      Ok(request) => request,
      Err(error) => {
        complete(Err(NativeCheckError::Compiler(format!(
          "cannot serialize OH SDK plugin request batch for JavaScript: {error}"
        ))));
        return;
      }
    };
    // Either scheduling fails or the JS callback completes. This one-shot slot
    // gives both paths ownership without holding a lock while invoking the host.
    let completion = Arc::new(std::sync::Mutex::new(Some(complete)));
    let callback = Arc::clone(&completion);
    let status = self.dispatch.call_with_return_value(
      request,
      ThreadsafeFunctionCallMode::NonBlocking,
      move |result, _env| {
        let result = result
          .map_err(|error| {
            NativeCheckError::Compiler(format!("OH SDK plugin callback batch threw: {error}"))
          })
          .and_then(|response| {
            serde_json::from_str(&response).map_err(|error| {
              NativeCheckError::Compiler(format!(
                "invalid OH SDK plugin batch response from JavaScript: {error}"
              ))
            })
          });
        let complete = callback.lock().unwrap().take();
        if let Some(complete) = complete {
          complete(result);
        }
        Ok(())
      },
    );
    if status != Status::Ok {
      let complete = completion.lock().unwrap().take();
      if let Some(complete) = complete {
        complete(Err(NativeCheckError::Compiler(format!(
          "failed to schedule OH SDK plugin callback batch: {status:?}"
        ))));
      }
    }
  }
}

/// Remove project, module, unit-test, preview, and active native build outputs.
/// All project discovery and path ownership remain in Rust.
#[napi]
pub async fn clean(config: CleanConfig) -> napi::Result<()> {
  let request = arkdown_build::clean::CleanRequest {
    project: config.project.map(PathBuf::from),
  };
  tokio::task::spawn_blocking(move || {
    arkdown_build::clean::CleanApplication::new(request).execute()
  })
  .await
  .map_err(|error| napi::Error::from_reason(format!("Arkdown clean task failed: {error}")))?
  .map_err(|error| napi::Error::from_reason(format!("{error:#}")))
}

/// Run the native ArkTS source checker without entering the build pipeline.
#[napi(skip_typescript)]
pub fn check<'a>(
  env: Env,
  config: CheckConfig,
  sdk_plugin_dispatch: Function<'a, (String,), String>,
) -> napi::Result<Object<'a>> {
  let sdk_plugin_host = Some(NapiSdkPluginHost::from_dispatch(sdk_plugin_dispatch)?);
  let args = arkdown_build::check::CheckRequest {
    target: config.target.unwrap_or_else(|| "hap".to_string()),
    module: config.module,
    project: config.project.map(PathBuf::from),
    is_crossplatform: config.is_crossplatform.unwrap_or(false),
    ignore_crossplatform_check: config.ignore_crossplatform_check.unwrap_or(false),

    sdk_plugin_host,
    heap_reclaimer: Some(heap_reclaimer::NapiHeapReclaimer::from_env(&env)?),
  };
  let raw_promise = napi::bindgen_prelude::execute_tokio_future(
    env.raw(),
    async move {
      tokio::task::spawn_blocking(move || {
        arkdown_build::check::CheckApplication::new(args).execute()
      })
      .await
      .map_err(|error| napi::Error::from_reason(format!("Arkdown check task failed: {error}")))?
      .map_err(|error| napi::Error::from_reason(format!("{error:#}")))?;
      Ok::<_, napi::Error>(())
    },
    |env_raw, result| unsafe { napi::bindgen_prelude::ToNapiValue::to_napi_value(env_raw, result) },
  )?;
  unsafe { Object::from_napi_value(env.raw(), raw_promise) }
}

/// Compile one user-authored TypeScript plugin module through OXC and Rolldown.
/// JavaScript receives only the CommonJS text it must execute in the owning V8
/// isolate.
#[napi]
pub async fn compile_plugin_module(file: String, source_map: Option<bool>) -> napi::Result<String> {
  plugin_module::PluginModuleNapiCompiler::compile(file, source_map.unwrap_or(false)).await
}

/// Synchronous counterpart required by Node's `Module._extensions` contract
/// for TypeScript files imported from another plugin module.
#[napi]
pub fn compile_plugin_module_sync(file: String, source_map: Option<bool>) -> napi::Result<String> {
  plugin_module::PluginModuleNapiCompiler::compile_sync(file, source_map.unwrap_or(false))
}

/// Compute a plugin module cache identity in Rust so the JavaScript adapter
/// does not own filesystem or fingerprinting behavior.
#[napi]
pub fn plugin_module_cache_identity(file: String) -> napi::Result<String> {
  plugin_module::PluginModuleNapiCompiler::cache_identity(file)
}

/// Resolve latest-hvigor compile-plugin order against the native Rolldown
/// pipeline. JavaScript functions are invoked synchronously on the owning V8
/// thread; all grouping, built-in inventory, validation, and slot reduction
/// remain in Rust.
#[napi]
pub fn plan_hvigor_compile_plugins<'env>(
  env: Env,
  config: JsHvigorCompilePluginPlanConfig,
  plugins: Array<'env>,
) -> napi::Result<serde_json::Value> {
  hvigor_plugin_plan::HvigorCompilePluginNapiPlanner::from_js(env, config.into(), plugins)?
    .plan_value()
}

/// Resolve the function-valued order extension for explicit package plugins.
/// String-valued first/last orders stay on the plugin for Rolldown's Rust
/// factory; only callback execution crosses into JavaScript.
#[napi]
pub fn plan_rolldown_package_plugins<'env>(
  env: Env,
  plugins: Array<'env>,
) -> napi::Result<serde_json::Value> {
  hvigor_plugin_plan::RolldownPackagePluginNapiPlanner::from_js(env, plugins)?.plan_value()
}

/// Validate raw compile-plugin exports before JavaScript wraps their function
/// identities into Rolldown hook adapters.
#[napi]
pub fn validate_hvigor_compile_plugins(
  plugins: Array<'_>,
  plugin_file: String,
) -> napi::Result<serde_json::Value> {
  hvigor_plugin_plan::HvigorPluginNapiValidator::validate_compile(plugins, plugin_file)
}

#[napi(object)]
pub struct JsHvigorCompilePluginPlanConfig {
  pub compile_mode: Option<String>,
  pub preview: Option<bool>,
  pub target: Option<String>,
  pub mode: Option<String>,
}

impl From<JsHvigorCompilePluginPlanConfig> for HvigorCompilePluginPlanConfig {
  fn from(value: JsHvigorCompilePluginPlanConfig) -> Self {
    Self {
      compile_mode: value.compile_mode,
      preview: value.preview.unwrap_or(false),
      target: value.target,
      mode: value.mode,
    }
  }
}

// ---------------------------------------------------------------------------
// Type aliases for the ThreadsafeFunction types
// ---------------------------------------------------------------------------

// CalleeHandled=false: build_callback returns this variant; call_async(value) not call_async(Ok(value))
#[napi(transparent)]
pub struct JsHookSideEffects(Either<bool, String>);

impl TryFrom<JsHookSideEffects> for HookSideEffects {
  type Error = anyhow::Error;

  fn try_from(value: JsHookSideEffects) -> std::result::Result<Self, Self::Error> {
    match value.0 {
      Either::A(true) => Ok(Self::True),
      Either::A(false) => Ok(Self::False),
      Either::B(value) if value == "no-treeshake" => Ok(Self::NoTreeshake),
      Either::B(value) => anyhow::bail!(
        "invalid moduleSideEffects value '{value}'; expected a boolean or 'no-treeshake'"
      ),
    }
  }
}

#[napi(transparent)]
pub struct JsResolvedExternal(Either<bool, String>);

impl TryFrom<JsResolvedExternal> for ResolvedExternal {
  type Error = anyhow::Error;

  fn try_from(value: JsResolvedExternal) -> std::result::Result<Self, Self::Error> {
    match value.0 {
      Either::A(value) => Ok(Self::Bool(value)),
      Either::B(value) if value == "absolute" => Ok(Self::Absolute),
      Either::B(value) if value == "relative" => Ok(Self::Relative),
      Either::B(value) => anyhow::bail!(
        "invalid external value '{value}'; expected a boolean, 'absolute', or 'relative'"
      ),
    }
  }
}

#[napi(object)]
pub struct JsResolveIdOptions {
  pub attributes: HashMap<String, String>,
  pub is_entry: bool,
  pub kind: Option<String>,
  pub custom: Option<serde_json::Value>,
}

#[napi(object)]
pub struct JsResolveIdOutput {
  pub id: String,
  pub external: Option<JsResolvedExternal>,
  pub normalize_external_id: Option<bool>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<JsHookSideEffects>,
  pub package_json_path: Option<String>,
  pub meta: Option<serde_json::Value>,
  pub description: Option<String>,
}

impl TryFrom<JsResolveIdOutput> for HookResolveIdOutput {
  type Error = anyhow::Error;

  fn try_from(value: JsResolveIdOutput) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      id: value.id.into(),
      external: value.external.map(TryInto::try_into).transpose()?,
      normalize_external_id: value.normalize_external_id,
      side_effects: value
        .module_side_effects
        .map(TryInto::try_into)
        .transpose()?,
      package_json_path: value.package_json_path,
    })
  }
}

impl From<ResolvedExternal> for JsResolvedExternal {
  fn from(value: ResolvedExternal) -> Self {
    Self(match value {
      ResolvedExternal::Bool(value) => Either::A(value),
      ResolvedExternal::Absolute => Either::B("absolute".to_string()),
      ResolvedExternal::Relative => Either::B("relative".to_string()),
    })
  }
}

impl From<HookSideEffects> for JsHookSideEffects {
  fn from(value: HookSideEffects) -> Self {
    Self(match value {
      HookSideEffects::True => Either::A(true),
      HookSideEffects::False => Either::A(false),
      HookSideEffects::NoTreeshake => Either::B("no-treeshake".to_string()),
    })
  }
}

#[napi(object)]
pub struct JsJsonSourceMap {
  pub file: Option<String>,
  pub source_root: Option<String>,
  pub sources: Option<Vec<Option<String>>>,
  pub sources_content: Option<Vec<Option<String>>>,
  pub names: Option<Vec<String>>,
  pub mappings: Option<String>,
  pub debug_id: Option<String>,
  #[napi(js_name = "x_google_ignoreList")]
  pub x_google_ignore_list: Option<Vec<u32>>,
}

impl TryFrom<JsJsonSourceMap> for rolldown_sourcemap::SourceMap {
  type Error = anyhow::Error;

  fn try_from(value: JsJsonSourceMap) -> std::result::Result<Self, Self::Error> {
    rolldown_sourcemap::SourceMap::from_json(rolldown_sourcemap::JSONSourceMap {
      version: 3,
      file: value.file,
      mappings: value.mappings.unwrap_or_default(),
      source_root: value.source_root,
      sources: value
        .sources
        .unwrap_or_default()
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect(),
      sources_content: value.sources_content,
      names: value.names.unwrap_or_default(),
      debug_id: value.debug_id,
      x_google_ignore_list: value.x_google_ignore_list,
    })
    .map_err(|error| anyhow::anyhow!("invalid source map: {error}"))
  }
}

#[napi(transparent)]
pub struct JsSourceMap(Either<String, JsJsonSourceMap>);

impl TryFrom<JsSourceMap> for rolldown_sourcemap::SourceMap {
  type Error = anyhow::Error;

  fn try_from(value: JsSourceMap) -> std::result::Result<Self, Self::Error> {
    match value.0 {
      Either::A(value) => rolldown_sourcemap::OwnedSourceMap::from_json_string(&value)
        .map(rolldown_sourcemap::OwnedSourceMap::into_inner)
        .map_err(|error| anyhow::anyhow!("invalid source map: {error}")),
      Either::B(value) => value.try_into(),
    }
  }
}

#[napi(object)]
pub struct JsLoadOutput {
  pub code: String,
  pub map: Option<JsSourceMap>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<JsHookSideEffects>,
  pub module_type: Option<String>,
  pub meta: Option<serde_json::Value>,
  pub description: Option<String>,
}

impl TryFrom<JsLoadOutput> for HookLoadOutput {
  type Error = anyhow::Error;

  fn try_from(value: JsLoadOutput) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      code: value.code.into(),
      map: value.map.map(TryInto::try_into).transpose()?,
      side_effects: value
        .module_side_effects
        .map(TryInto::try_into)
        .transpose()?,
      module_type: value
        .module_type
        .map(|value| ModuleType::from_str_with_fallback(&value)),
    })
  }
}

#[napi(object)]
pub struct JsTransformOutput {
  pub code: Option<String>,
  pub map: Option<Either<JsSourceMap, Null>>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<JsHookSideEffects>,
  pub module_type: Option<String>,
  pub meta: Option<serde_json::Value>,
  pub description: Option<String>,
}

impl TryFrom<JsTransformOutput> for HookTransformOutput {
  type Error = anyhow::Error;

  fn try_from(value: JsTransformOutput) -> std::result::Result<Self, Self::Error> {
    let map = match value.map {
      Some(Either::A(map)) => HookTransformOutputMap::Sourcemap(Box::new(map.try_into()?)),
      Some(Either::B(_)) => HookTransformOutputMap::Null,
      None => HookTransformOutputMap::Omitted,
    };
    Ok(Self {
      code: value.code,
      map,
      side_effects: value
        .module_side_effects
        .map(TryInto::try_into)
        .transpose()?,
      module_type: value
        .module_type
        .map(|value| ModuleType::from_str_with_fallback(&value)),
    })
  }
}

/// Narrow NAPI view over Rolldown's native plugin context.
///
/// Keeping the native context live is both more accurate and substantially
/// cheaper than serializing the complete module graph for every hook. It also
/// preserves Rolldown's `getFileName` ownership and diagnostics verbatim.
#[napi]
pub struct ArkdownRolldownPluginContext {
  inner: PluginContext,
  load_context: Option<SharedLoadPluginContext>,
  transform_context: Option<SharedTransformPluginContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ArkdownJsPluginResolveCustom;

impl TypedMapKey for ArkdownJsPluginResolveCustom {
  type Value = serde_json::Value;
}

#[napi(object)]
pub struct JsPluginContextResolveOptions {
  #[napi(
    ts_type = "'import-statement' | 'dynamic-import' | 'require-call' | 'import-rule' | 'url-token' | 'new-url' | 'hot-accept'"
  )]
  pub kind: Option<String>,
  pub is_entry: Option<bool>,
  pub skip_self: Option<bool>,
  pub custom: Option<serde_json::Value>,
}

#[napi(object)]
pub struct JsPluginContextLoadOptions {
  pub id: String,
  pub resolve_dependencies: Option<bool>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<JsHookSideEffects>,
  pub meta: Option<serde_json::Value>,
  pub description: Option<String>,
}

#[napi(object)]
pub struct JsEmittedFile {
  pub r#type: String,
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub original_file_name: Option<String>,
  #[napi(ts_type = "string | Uint8Array")]
  pub source: Option<Either<String, Buffer>>,
  pub id: Option<String>,
  pub importer: Option<String>,
  #[napi(ts_type = "'strict' | 'allow-extension' | 'exports-only' | false")]
  pub preserve_signature: Option<Either<String, bool>>,
}

impl JsEmittedFile {
  fn into_asset(self) -> napi::Result<EmittedAsset> {
    let source = self.source.ok_or_else(|| {
      napi::Error::from_reason("emitFile asset requires a string or Uint8Array source")
    })?;
    Ok(EmittedAsset {
      name: self.name,
      original_file_name: self.original_file_name,
      file_name: self.file_name.map(Into::into),
      source: match source {
        Either::A(source) => StrOrBytes::Str(source),
        Either::B(source) => StrOrBytes::Bytes(source.to_vec()),
      },
    })
  }

  fn into_chunk(self) -> napi::Result<EmittedChunk> {
    let preserve_entry_signatures = match self.preserve_signature {
      Some(Either::A(value)) if value == "strict" => Some(PreserveEntrySignatures::Strict),
      Some(Either::A(value)) if value == "allow-extension" => {
        Some(PreserveEntrySignatures::AllowExtension)
      }
      Some(Either::A(value)) if value == "exports-only" => {
        Some(PreserveEntrySignatures::ExportsOnly)
      }
      Some(Either::A(value)) => {
        return Err(napi::Error::from_reason(format!(
          "invalid emitted chunk preserveSignature '{value}'"
        )))
      }
      Some(Either::B(false)) => Some(PreserveEntrySignatures::False),
      Some(Either::B(true)) => {
        return Err(napi::Error::from_reason(
          "emitted chunk preserveSignature only accepts false or a supported string",
        ))
      }
      None => None,
    };
    Ok(EmittedChunk {
      name: self.name.map(Into::into),
      file_name: self.file_name.map(Into::into),
      id: self
        .id
        .ok_or_else(|| napi::Error::from_reason("emitFile chunk requires an id"))?,
      importer: self.importer,
      preserve_entry_signatures,
    })
  }
}

#[napi(object)]
pub struct JsPluginAstParseOptions {
  pub allow_return_outside_function: Option<bool>,
}

impl From<JsPluginAstParseOptions> for arkdown::PluginAstParseOptions {
  fn from(options: JsPluginAstParseOptions) -> Self {
    Self {
      allow_return_outside_function: options.allow_return_outside_function.unwrap_or(false),
    }
  }
}

impl From<PluginContext> for ArkdownRolldownPluginContext {
  fn from(inner: PluginContext) -> Self {
    Self {
      inner,
      load_context: None,
      transform_context: None,
    }
  }
}

impl ArkdownRolldownPluginContext {
  fn from_load(inner: SharedLoadPluginContext) -> Self {
    Self {
      inner: inner.inner.clone(),
      load_context: Some(inner),
      transform_context: None,
    }
  }

  fn from_transform(inner: SharedTransformPluginContext) -> Self {
    Self {
      inner: inner.inner.clone(),
      load_context: None,
      transform_context: Some(inner),
    }
  }
}

struct ArkdownPluginAstBridge;

impl ArkdownPluginAstBridge {
  fn parse(
    source: &str,
    options: Option<JsPluginAstParseOptions>,
  ) -> napi::Result<serde_json::Value> {
    arkdown::PluginAstParser::new(source, options.map(Into::into).unwrap_or_default())
      .parse()
      .map(arkdown::PluginAstParseResult::into_bridge_value)
      .map_err(|error| napi::Error::from_reason(error.to_string()))
  }
}

#[napi]
impl ArkdownRolldownPluginContext {
  #[napi]
  pub async fn resolve(
    &self,
    specifier: String,
    importer: Option<String>,
    options: Option<JsPluginContextResolveOptions>,
  ) -> napi::Result<Option<JsResolveIdOutput>> {
    let options = options.unwrap_or(JsPluginContextResolveOptions {
      kind: None,
      is_entry: None,
      skip_self: None,
      custom: None,
    });
    let mut custom = CustomField::new();
    if let Some(value) = options.custom {
      custom.insert(ArkdownJsPluginResolveCustom, value);
    }
    let import_kind = options
      .kind
      .as_deref()
      .unwrap_or("import-statement")
      .try_into()
      .map_err(napi::Error::from_reason)?;
    let result = self
      .inner
      .resolve(
        &specifier,
        importer.as_deref(),
        Some(PluginContextResolveOptions {
          import_kind,
          is_entry: options.is_entry.unwrap_or(importer.is_none()),
          skip_self: options.skip_self.unwrap_or(true),
          custom: Arc::new(custom),
        }),
      )
      .await
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    Ok(result.ok().map(|resolved| {
      JsResolveIdOutput {
        id: resolved.id.to_string(),
        external: Some(resolved.external.into()),
        normalize_external_id: resolved.normalize_external_id,
        module_side_effects: resolved.side_effects.map(Into::into),
        package_json_path: resolved
          .package_json
          .map(|package| package.realpath().to_string_lossy().into_owned()),
        meta: None,
        description: None,
      }
    }))
  }

  #[napi]
  pub async fn load(
    &self,
    options: JsPluginContextLoadOptions,
  ) -> napi::Result<Option<HvigorModuleInfo>> {
    let id = options.id;
    self
      .inner
      .load(
        &id,
        options
          .module_side_effects
          .map(TryInto::try_into)
          .transpose()
          .map_err(|error: anyhow::Error| napi::Error::from_reason(error.to_string()))?,
        ModuleDefFormat::Unknown,
      )
      .await
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    Ok(
      self
        .inner
        .get_module_info(&id)
        .map(|info| HvigorModuleInfo::from_rolldown(&info)),
    )
  }

  #[napi]
  pub fn emit_file(&self, file: JsEmittedFile) -> napi::Result<String> {
    let file_type = file.r#type.clone();
    let reference_id = match file_type.as_str() {
      "asset" => self.inner.emit_file(file.into_asset()?, None, None),
      "chunk" => self.inner.emit_chunk(file.into_chunk()?),
      _ => {
        return Err(napi::Error::from_reason(format!(
          "emitFile type must be 'asset' or 'chunk', got '{file_type}'"
        )))
      }
    }
    .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    Ok(reference_id.to_string())
  }

  #[napi]
  pub fn add_watch_file(&self, id: String) {
    if let Some(context) = &self.transform_context {
      context.add_watch_file(&id);
    } else if let Some(context) = &self.load_context {
      context.add_watch_file(&id);
    } else {
      self.inner.add_watch_file(&id);
    }
  }

  /// Rolldown's transform-context source map chain, including every load and
  /// transform result that ran before the current JavaScript plugin.
  #[napi]
  pub fn get_combined_sourcemap(&self) -> napi::Result<serde_json::Value> {
    let context = self.transform_context.as_ref().ok_or_else(|| {
      napi::Error::from_reason("getCombinedSourcemap is only available inside transform hooks")
    })?;
    serde_json::from_str(&context.get_combined_sourcemap().to_json_string())
      .map_err(|error| napi::Error::from_reason(format!("invalid combined source map: {error}")))
  }

  #[napi]
  pub fn info(&self, message: String) {
    self.inner.info(LogWithoutPlugin {
      message,
      code: Some("PLUGIN_LOG".to_string()),
      ..Default::default()
    });
  }

  #[napi]
  pub fn warn(&self, message: String) {
    self.inner.warn(LogWithoutPlugin {
      message,
      code: Some("PLUGIN_WARNING".to_string()),
      ..Default::default()
    });
  }

  #[napi]
  pub fn debug(&self, message: String) {
    self.inner.debug(LogWithoutPlugin {
      message,
      code: Some("PLUGIN_LOG".to_string()),
      ..Default::default()
    });
  }

  #[napi]
  pub fn get_module_ids(&self) -> Vec<String> {
    self
      .inner
      .get_module_ids()
      .into_iter()
      .map(|id| id.to_string())
      .collect()
  }

  #[napi]
  pub fn get_module_info(&self, id: String) -> Option<HvigorModuleInfo> {
    self
      .inner
      .get_module_info(&id)
      .map(|info| HvigorModuleInfo::from_rolldown(&info))
  }

  #[napi]
  pub fn get_file_name(&self, reference_id: String) -> napi::Result<String> {
    self
      .inner
      .get_file_name(&reference_id)
      .map(|name| name.to_string())
      .map_err(|error| napi::Error::from_reason(error.to_string()))
  }

  /// OXC-backed equivalent of Rollup's synchronous `PluginContext.parse`.
  #[napi]
  pub fn parse(
    &self,
    source: String,
    options: Option<JsPluginAstParseOptions>,
  ) -> napi::Result<serde_json::Value> {
    ArkdownPluginAstBridge::parse(&source, options)
  }
}

#[napi(object)]
pub struct JsBuildStartOptions {
  #[napi(ts_type = "string | string[] | Record<string, string>")]
  pub input: Either3<String, Vec<String>, HashMap<String, String>>,
  pub cwd: String,
  pub platform: String,
  pub shim_missing_exports: bool,
  pub context: String,
}

#[napi(object)]
pub struct JsTransformHookExtraArgs {
  pub module_type: String,
}

#[napi(object)]
pub struct JsPluginError {
  pub message: String,
  pub id: Option<String>,
  pub plugin: Option<String>,
}

#[napi(object)]
pub struct JsBuildEndContext {
  pub errors: Vec<JsPluginError>,
}

#[napi(object, object_from_js = false)]
pub struct JsPluginDispatchRequest {
  pub hook_name: String,
  pub context: Option<ArkdownRolldownPluginContext>,
  pub build_start_options: Option<JsBuildStartOptions>,
  pub id: Option<String>,
  pub importer: Option<String>,
  pub watch_event: Option<String>,
  pub resolve_options: Option<JsResolveIdOptions>,
  pub code: Option<String>,
  pub transform_options: Option<JsTransformHookExtraArgs>,
  pub module_info: Option<HvigorModuleInfo>,
  pub build_end_context: Option<JsBuildEndContext>,
  pub cache_request: Option<serde_json::Value>,
  pub share: Option<HvigorShareContext>,
}

impl JsPluginDispatchRequest {
  fn new(hook_name: &str, context: Option<ArkdownRolldownPluginContext>) -> Self {
    Self {
      hook_name: hook_name.to_string(),
      context,
      build_start_options: None,
      id: None,
      importer: None,
      watch_event: None,
      resolve_options: None,
      code: None,
      transform_options: None,
      module_info: None,
      build_end_context: None,
      cache_request: None,
      share: None,
    }
  }
}

#[napi(object, object_to_js = false)]
pub struct JsPluginDispatchResult {
  pub resolve_id: Option<Either3<String, JsResolveIdOutput, bool>>,
  pub load: Option<Either<String, JsLoadOutput>>,
  pub transform: Option<Either<String, JsTransformOutput>>,
  pub should_invalid_cache: Option<bool>,
  #[napi(js_name = "cacheSnapshot", skip_typescript)]
  pub cache_snapshot: Option<Buffer>,
}

#[napi(object)]
pub struct JsPluginHookMetaInput {
  pub name: String,
  pub order: String,
}

type PluginDispatchTsfn = MaybeAsyncJsCallback<
  JsPluginDispatchRequest,
  FnArgs<(JsPluginDispatchRequest,)>,
  JsPluginDispatchResult,
>;

// ---------------------------------------------------------------------------
// JsPlugin — bridges a JavaScript plugin object to rolldown_plugin::Plugin
// ---------------------------------------------------------------------------

/// Wraps a JavaScript plugin object.
///
/// One typed dispatcher per plugin can be called from any Rust thread
/// (including `tokio` async tasks). Rust owns hook usage and scheduling while
/// JavaScript retains only the live function receiver/context bridge.
pub struct JsPlugin {
  name: String,
  module_root: Option<PathBuf>,
  target_name: Option<String>,
  dispatch_fn: PluginDispatchTsfn,
  hook_names: Vec<String>,
  hook_orders: HashMap<String, JsHookOrder>,
  cache_identity: String,
  usage: HookUsage,
  watch_changes: Mutex<Arc<[HvigorWatchChange]>>,
}

#[derive(Clone, Copy)]
enum JsHookOrder {
  Pre,
  Post,
}

impl std::fmt::Debug for JsPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("JsPlugin")
      .field("name", &self.name)
      .finish_non_exhaustive()
  }
}

impl JsPlugin {
  fn has_hook(&self, hook_name: &str) -> bool {
    self.hook_names.iter().any(|name| name == hook_name)
  }

  fn hook_meta(&self, hook_name: &str) -> Option<PluginHookMeta> {
    self.hook_orders.get(hook_name).map(|order| PluginHookMeta {
      order: Some(match order {
        JsHookOrder::Pre => RolldownPluginOrder::Pre,
        JsHookOrder::Post => RolldownPluginOrder::Post,
      }),
    })
  }

  fn resolve_output(
    output: Either3<String, JsResolveIdOutput, bool>,
    specifier: String,
  ) -> anyhow::Result<HookResolveIdOutput> {
    match output {
      Either3::A(id) => Ok(HookResolveIdOutput::from_id(id)),
      Either3::B(output) => output.try_into(),
      Either3::C(false) => Ok(HookResolveIdOutput {
        id: specifier.into(),
        external: Some(ResolvedExternal::Bool(true)),
        ..Default::default()
      }),
      Either3::C(true) => anyhow::bail!(
        "resolve hook returned true; expected a string, an object, false, null, or undefined"
      ),
    }
  }

  fn build_start_options(args: &HookBuildStartArgs<'_>) -> JsBuildStartOptions {
    let input = if args.options.input.len() == 1 && args.options.input[0].name.is_none() {
      Either3::A(args.options.input[0].import.clone())
    } else if args.options.input.iter().all(|item| item.name.is_some()) {
      Either3::C(
        args
          .options
          .input
          .iter()
          .filter_map(|item| {
            item
              .name
              .as_ref()
              .map(|name| (name.clone(), item.import.clone()))
          })
          .collect(),
      )
    } else {
      Either3::B(
        args
          .options
          .input
          .iter()
          .map(|item| item.import.clone())
          .collect(),
      )
    };
    JsBuildStartOptions {
      input,
      cwd: args.options.cwd.to_string_lossy().into_owned(),
      platform: args.options.platform.to_string(),
      shim_missing_exports: args.options.shim_missing_exports,
      context: args.options.context.clone(),
    }
  }

  fn from_js_object(obj: Object<'_>) -> napi::Result<(Arc<Self>, HvigorPluginOrder)> {
    let name: String = obj
      .get::<String>("name")?
      .unwrap_or_else(|| "unknown".to_string());
    let cache_identity = obj
      .get::<String>("__arkdownPluginCacheIdentity")?
      .unwrap_or_else(|| name.clone());
    let module_root = obj.get::<String>("__arkdownModuleRoot")?.map(PathBuf::from);
    let target_name = obj.get::<String>("__arkdownTargetName")?;
    let anchor_rank = obj.get::<u32>("__arkdownOrderAnchorRank")?;
    let anchor_sequence = obj.get::<u32>("__arkdownOrderAnchorSequence")?;
    let order = if let (Some(rank), Some(sequence)) = (anchor_rank, anchor_sequence) {
      let rank = u16::try_from(rank)
        .map_err(|_| napi::Error::from_reason("compile plugin anchor rank exceeds u16"))?;
      if rank > HvigorNativePluginSlot::END_RANK {
        return Err(napi::Error::from_reason(format!(
          "compile plugin anchor rank {rank} exceeds native pipeline end rank {}",
          HvigorNativePluginSlot::END_RANK
        )));
      }
      HvigorPluginOrder::Anchor { rank, sequence }
    } else if anchor_rank.is_some() || anchor_sequence.is_some() {
      return Err(napi::Error::from_reason(
        "compile plugin anchor rank and sequence must be supplied together",
      ));
    } else if let Some(position) = obj.get::<u32>("__arkdownOrderIndex")? {
      HvigorPluginOrder::Position(position as usize)
    } else {
      match obj.get::<String>("order")?.as_deref() {
        Some("first") => HvigorPluginOrder::First,
        Some("last") | None => HvigorPluginOrder::Last,
        Some(other) => {
          tracing::warn!(
            plugin = %name,
            order = %other,
            "unsupported JS plugin order, using 'last'"
          );
          HvigorPluginOrder::Last
        }
      }
    };

    let hook_names = obj
      .get::<Vec<String>>("__arkdownPluginHooks")?
      .ok_or_else(|| napi::Error::from_reason(format!("plugin '{name}' has no Rust hook plan")))?;
    let hook_meta = obj
      .get::<Vec<JsPluginHookMetaInput>>("__arkdownPluginHookMeta")?
      .unwrap_or_default();
    let mut hook_orders = HashMap::new();
    for hook in hook_meta {
      let order = match hook.order.as_str() {
        "pre" => JsHookOrder::Pre,
        "post" => JsHookOrder::Post,
        other => {
          return Err(napi::Error::from_reason(format!(
            "plugin '{name}' hook '{}' has unsupported order '{other}'",
            hook.name
          )))
        }
      };
      hook_orders.insert(hook.name, order);
    }
    let mut usage = HookUsage::empty();
    for hook_name in &hook_names {
      usage |= match hook_name.as_str() {
        "buildStart" => HookUsage::BuildStart,
        "resolveId" => HookUsage::ResolveId,
        "resolveDynamicImport" => HookUsage::ResolveDynamicImport,
        "load" => HookUsage::Load,
        "transform" => HookUsage::Transform,
        "moduleParsed" => HookUsage::ModuleParsed,
        "buildEnd" => HookUsage::BuildEnd,
        // Arkdown's Rust watcher owns filesystem observation. Its internal
        // lifecycle adapter forwards `watchChange` with a live Rolldown
        // context before the next `beforeBuild`, so this JS plugin must not
        // register a second Rolldown watcher hook.
        "watchChange" => HookUsage::empty(),
        "beforeBuild"
        | "afterBuildStart"
        | "beforeBuildEnd"
        | "afterBuildEnd"
        | "bundleEnd"
        | "cleanUp"
        | "shouldInvalidCache"
        | "renderStart"
        | "banner"
        | "footer"
        | "intro"
        | "outro"
        | "renderDynamicImport"
        | "resolveFileUrl"
        | "resolveImportMeta"
        | "renderChunk"
        | "augmentChunkHash"
        | "renderError"
        | "generateBundle"
        | "writeBundle"
        | "closeBundle"
        | "closeWatcher" => HookUsage::empty(),
        other => {
          return Err(napi::Error::from_reason(format!(
            "plugin '{name}' has unsupported hook '{other}'"
          )))
        }
      };
    }
    let dispatch = obj
      .get::<Function<(JsPluginDispatchRequest,), MaybeAsyncJsReturn<JsPluginDispatchResult>>>(
        "__arkdownDispatchPluginHook",
      )?
      .ok_or_else(|| {
        napi::Error::from_reason(format!("plugin '{name}' has no Rust hook dispatcher"))
      })?;
    let dispatch_fn = Arc::new(
      dispatch
        .build_threadsafe_function::<JsPluginDispatchRequest>()
        .weak::<true>()
        .build_callback(|ctx: ThreadsafeCallContext<JsPluginDispatchRequest>| {
          Ok(FnArgs { data: (ctx.value,) })
        })?,
    );

    tracing::debug!(
      plugin = %name,
      module_root = ?module_root,
      target_name = ?target_name,
      "wrapped JS plugin"
    );
    Ok((
      Arc::new(Self {
        name,
        module_root,
        target_name,
        dispatch_fn,
        hook_names,
        hook_orders,
        cache_identity,
        usage,
        watch_changes: Mutex::new(Arc::from([])),
      }),
      order,
    ))
  }
}

// ---------------------------------------------------------------------------
// Plugin trait implementation
// ---------------------------------------------------------------------------

impl Plugin for JsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Owned(self.name.clone())
  }

  fn register_hook_usage(&self) -> HookUsage {
    self.usage
  }

  fn build_start(
    &self,
    ctx: &PluginContext,
    args: &HookBuildStartArgs<'_>,
  ) -> impl std::future::Future<Output = rolldown_plugin::HookNoopReturn> + Send {
    let mut request = JsPluginDispatchRequest::new("buildStart", Some(ctx.clone().into()));
    request.build_start_options = Some(Self::build_start_options(args));
    let dispatch = self.dispatch_fn.clone();
    async move {
      dispatch
        .await_call(request)
        .await
        .map_err(|e| anyhow::anyhow!("buildStart JS error: {e}"))?;
      Ok(())
    }
  }

  fn build_start_meta(&self) -> Option<PluginHookMeta> {
    self.hook_meta("buildStart")
  }

  fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
    let specifier = args.specifier.to_string();
    let importer = args.importer.map(|s| s.to_string());
    let options = JsResolveIdOptions {
      attributes: HashMap::new(),
      is_entry: args.is_entry,
      kind: Some(args.kind.to_string()),
      custom: args
        .custom
        .get::<ArkdownJsPluginResolveCustom>(&ArkdownJsPluginResolveCustom)
        .cloned(),
    };
    let mut request = JsPluginDispatchRequest::new("resolveId", Some(ctx.clone().into()));
    request.id = Some(specifier.clone());
    request.importer = importer;
    request.resolve_options = Some(options);
    let dispatch = self.dispatch_fn.clone();
    async move {
      match dispatch.await_call(request).await {
        Ok(result) => result
          .resolve_id
          .map(|output| Self::resolve_output(output, specifier))
          .transpose(),
        Err(error) => Err(anyhow::anyhow!("resolveId JS error: {error}")),
      }
    }
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    self.hook_meta("resolveId")
  }

  /// `resolveDynamicImport` — bridges dynamic import resolution to JS.
  ///
  /// Same dispatcher envelope as `resolveId`. Latest Hvigor excludes this
  /// hook from compile-plugin `doWrapper`; it remains available only to the
  /// explicit Rolldown plugin API.
  fn resolve_dynamic_import(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
    let specifier = args.specifier.to_string();
    let importer = args.importer.map(|s| s.to_string());
    let options = JsResolveIdOptions {
      attributes: HashMap::new(),
      is_entry: args.is_entry,
      kind: Some(args.kind.to_string()),
      custom: args
        .custom
        .get::<ArkdownJsPluginResolveCustom>(&ArkdownJsPluginResolveCustom)
        .cloned(),
    };
    let mut request =
      JsPluginDispatchRequest::new("resolveDynamicImport", Some(ctx.clone().into()));
    request.id = Some(specifier.clone());
    request.importer = importer;
    request.resolve_options = Some(options);
    let dispatch = self.dispatch_fn.clone();
    async move {
      match dispatch.await_call(request).await {
        Ok(result) => result
          .resolve_id
          .map(|output| Self::resolve_output(output, specifier))
          .transpose(),
        Err(error) => Err(anyhow::anyhow!("resolveDynamicImport JS error: {error}")),
      }
    }
  }

  fn resolve_dynamic_import_meta(&self) -> Option<PluginHookMeta> {
    self.hook_meta("resolveDynamicImport")
  }

  fn load(
    &self,
    ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    let id = args.id.to_string();
    let mut request =
      JsPluginDispatchRequest::new("load", Some(ArkdownRolldownPluginContext::from_load(ctx)));
    request.id = Some(id);
    let dispatch = self.dispatch_fn.clone();
    async move {
      match dispatch.await_call(request).await {
        Ok(result) => match result.load {
          Some(Either::A(code)) => Ok(Some(HookLoadOutput {
            code: code.into(),
            ..Default::default()
          })),
          Some(Either::B(output)) => Ok(Some(output.try_into()?)),
          None => Ok(None),
        },
        Err(error) => Err(anyhow::anyhow!("load JS error: {error}")),
      }
    }
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    self.hook_meta("load")
  }

  fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
    let mut request = JsPluginDispatchRequest::new(
      "transform",
      Some(ArkdownRolldownPluginContext::from_transform(ctx)),
    );
    request.code = Some(args.code.to_string());
    request.id = Some(args.id.to_string());
    request.transform_options = Some(JsTransformHookExtraArgs {
      module_type: args.module_type.to_string(),
    });
    let dispatch = self.dispatch_fn.clone();
    async move {
      match dispatch.await_call(request).await {
        Ok(result) => match result.transform {
          Some(Either::A(new_code)) => Ok(Some(HookTransformOutput {
            code: Some(new_code),
            ..Default::default()
          })),
          Some(Either::B(output)) => Ok(Some(output.try_into()?)),
          None => Ok(None),
        },
        Err(error) => Err(anyhow::anyhow!("transform JS error: {error}")),
      }
    }
  }

  fn transform_meta(&self) -> Option<PluginHookMeta> {
    self.hook_meta("transform")
  }

  fn module_parsed(
    &self,
    ctx: &PluginContext,
    module_info: Arc<ModuleInfo>,
    _normal_module: &NormalModule,
  ) -> impl std::future::Future<Output = rolldown_plugin::HookNoopReturn> + Send {
    let mut request = JsPluginDispatchRequest::new("moduleParsed", Some(ctx.clone().into()));
    request.module_info = Some(HvigorModuleInfo::from_rolldown(&module_info));
    let dispatch = self.dispatch_fn.clone();
    async move {
      dispatch
        .await_call(request)
        .await
        .map_err(|error| anyhow::anyhow!("moduleParsed JS error: {error}"))?;
      Ok(())
    }
  }

  fn module_parsed_meta(&self) -> Option<PluginHookMeta> {
    self.hook_meta("moduleParsed")
  }

  fn build_end(
    &self,
    ctx: &PluginContext,
    args: Option<&HookBuildEndArgs<'_>>,
  ) -> impl std::future::Future<Output = rolldown_plugin::HookNoopReturn> + Send {
    // Keep Rolldown's native context alive through the JavaScript callback.
    // This matches the latest hvigor wrapper and avoids cloning every module's
    // source into an intermediate JSON graph for each compile plugin.
    let errors = args.map_or_else(Vec::new, |args| {
      args
        .errors
        .iter()
        .map(|error| JsPluginError {
          message: error.to_string(),
          id: error.id(),
          plugin: error.plugin(),
        })
        .collect::<Vec<_>>()
    });
    let mut request = JsPluginDispatchRequest::new("buildEnd", Some(ctx.clone().into()));
    request.build_end_context = Some(JsBuildEndContext { errors });
    let dispatch = self.dispatch_fn.clone();
    async move {
      dispatch
        .await_call(request)
        .await
        .map_err(|error| anyhow::anyhow!("buildEnd JS error: {error}"))?;
      Ok(())
    }
  }

  fn build_end_meta(&self) -> Option<PluginHookMeta> {
    self.hook_meta("buildEnd")
  }
}

impl HvigorPluginGapHooks for JsPlugin {
  fn seed_hvigor_context<'a>(
    &'a self,
    share: &'a HvigorCompilePluginShare,
  ) -> HvigorHookFuture<'a, anyhow::Result<()>> {
    Box::pin(async move {
      *self
        .watch_changes
        .lock()
        .map_err(|_| anyhow::anyhow!("watch-change state lock is poisoned"))? =
        Arc::clone(&share.watch_changes);
      let mut request = JsPluginDispatchRequest::new("seedContext", None);
      request.share = Some(HvigorShareContext::from(share));
      self
        .dispatch_fn
        .await_call(request)
        .await
        .map_err(|error| anyhow::anyhow!("seed hvigor compile context JS error: {error}"))?;
      Ok(())
    })
  }

  fn should_invalid_cache<'a>(
    &'a self,
    module: &'a PluginCacheModule,
  ) -> HvigorHookFuture<'a, anyhow::Result<bool>> {
    Box::pin(async move {
      if !self.has_hook("shouldInvalidCache") {
        return Ok(false);
      }
      // @ohos/hvigor-arkts-compose 6.24.3 ShouldInvalidCacheOptions is exactly
      // `{ ast?: AcornNode, code: string, id: string }`. OXC constructs the AST
      // here in Rust; JavaScript only revives JSON-inexpressible literal values.
      let mut request = JsPluginDispatchRequest::new("shouldInvalidCache", None);
      request.cache_request = Some(module.hook_request_value());
      self
        .dispatch_fn
        .await_call(request)
        .await
        .map(|result| result.should_invalid_cache.unwrap_or(false))
        .map_err(|error| anyhow::anyhow!("shouldInvalidCache JS error: {error}"))
    })
  }

  fn has_should_invalid_cache(&self) -> bool {
    self.has_hook("shouldInvalidCache")
  }

  fn snapshot_hvigor_context_cache(&self) -> HvigorHookFuture<'_, anyhow::Result<Option<Vec<u8>>>> {
    Box::pin(async move {
      let request = JsPluginDispatchRequest::new("snapshotContext", None);
      self
        .dispatch_fn
        .await_call(request)
        .await
        .map(|result| result.cache_snapshot.map(|buffer| buffer.to_vec()))
        .map_err(|error| anyhow::anyhow!("snapshot plugin context JS error: {error}"))
    })
  }

  fn take_watch_changes(&self) -> anyhow::Result<Arc<[HvigorWatchChange]>> {
    let mut changes = self
      .watch_changes
      .lock()
      .map_err(|_| anyhow::anyhow!("watch-change state lock is poisoned"))?;
    Ok(std::mem::replace(&mut *changes, Arc::from([])))
  }

  fn watch_change_with_context(
    &self,
    change: HvigorWatchChange,
    context: PluginContext,
  ) -> HvigorHookFuture<'_, anyhow::Result<()>> {
    Box::pin(async move {
      let event = change.event.as_str();
      let path = arkdown_path::ArtifactPathSerializer::path_to_slash_string(&change.path)
        .map_err(|error| anyhow::anyhow!("serialize watchChange path failed: {error}"))?;
      let mut request = JsPluginDispatchRequest::new("watchChange", Some(context.into()));
      request.id = Some(path.clone());
      request.watch_event = Some(event.to_string());
      self
        .dispatch_fn
        .await_call(request)
        .await
        .map_err(|error| {
          anyhow::anyhow!("watchChange JS error for path '{path}' event '{event}': {error}")
        })?;
      Ok(())
    })
  }

  fn has_watch_change(&self) -> bool {
    self.has_hook("watchChange")
  }

  fn watch_change_order(&self) -> HvigorHookOrder {
    match self.hook_orders.get("watchChange") {
      Some(JsHookOrder::Pre) => HvigorHookOrder::Pre,
      Some(JsHookOrder::Post) => HvigorHookOrder::Post,
      None => HvigorHookOrder::Normal,
    }
  }

  fn lifecycle_hook_with_context(
    &self,
    hook: HvigorLifecycleHook,
    context: PluginContext,
  ) -> HvigorHookFuture<'_, anyhow::Result<()>> {
    Box::pin(async move {
      let hook_name = hook.as_str();
      if !self.has_hook(hook_name) {
        return Ok(());
      }
      let request = JsPluginDispatchRequest::new(hook_name, Some(context.into()));
      self
        .dispatch_fn
        .await_call(request)
        .await
        .map_err(|error| anyhow::anyhow!("{hook_name} JS error: {error}"))?;
      Ok(())
    })
  }

  fn has_lifecycle_hook(&self, hook: HvigorLifecycleHook) -> bool {
    self.has_hook(hook.as_str())
  }

  fn lifecycle_hook_order(&self, hook: HvigorLifecycleHook) -> HvigorHookOrder {
    match self.hook_orders.get(hook.as_str()) {
      Some(JsHookOrder::Pre) => HvigorHookOrder::Pre,
      Some(JsHookOrder::Post) => HvigorHookOrder::Post,
      None => HvigorHookOrder::Normal,
    }
  }

  fn runtime_cache_identity(&self) -> Option<String> {
    (!self.hook_names.is_empty())
      .then(|| format!("{}:{}", self.cache_identity, self.hook_names.join(",")))
  }
}

// ---------------------------------------------------------------------------
// NAPI-exposed config and result types
// ---------------------------------------------------------------------------

/// Project config metadata available as `this.share.projectConfig` inside plugin hooks.
///
/// Rust builds the complete effective object; the JavaScript adapter only
/// restores collection types required by the TS/JS plugin ABI.
#[derive(Clone)]
#[napi(object)]
pub struct HvigorProjectConfig {
  pub bundle_name: Option<String>,
  pub module_type: Option<String>,
  /// ets-loader AnalyzeMode: 0 default, 1 verbose, 2 false, 3 trace.
  pub perf: Option<u8>,
  pub build_mode: Option<String>,
  pub build_ark_mode: Option<String>,
  pub compile_mode: Option<String>,
  pub target: Option<String>,
  pub target_name: Option<String>,
  #[napi(js_name = "useNormalizedOHMUrl")]
  pub use_normalized_ohm_url: Option<bool>,
  pub compile_sdk_version: Option<u32>,
  pub compatible_sdk_version: Option<u32>,
  pub compatible_sdk_version_stage: Option<String>,
  pub package_manager_type: Option<String>,
  pub is_byte_code_har_optimize: Option<bool>,
  pub module_name: Option<String>,
  pub entry_module_name: Option<String>,
  pub entry_module_version: Option<String>,
  pub entry_package_name: Option<String>,
  pub module_path: Option<String>,
  pub project_top_dir: Option<String>,
  pub project_root_path: Option<String>,
  pub project_path: Option<String>,
  pub module_root_path: Option<String>,
  pub cache_path: Option<String>,
  pub ace_module_build: Option<String>,
  pub ace_build_json: Option<String>,
  pub ace_module_json_path: Option<String>,
  #[napi(js_name = "runtimeOS")]
  pub runtime_os: Option<String>,
  pub ets_loader_path: Option<String>,
  pub ets_loader_version: Option<String>,
  pub watch_mode: Option<String>,
  pub widget_compile: Option<String>,
  #[napi(js_name = "arkTSVersion")]
  pub ark_ts_version: Option<String>,
  pub is_preview: Option<bool>,
  pub is_ohos_test: Option<bool>,
  pub is_local_test: Option<bool>,
  pub is_crossplatform: Option<bool>,
  pub compile_har: Option<bool>,
  pub compile_shared: Option<bool>,
  pub byte_code_har: Option<bool>,
  pub single_file_emit: Option<bool>,
  pub need_coverage_insert: Option<bool>,
  pub need_complete_sources_map: Option<bool>,
  pub integrated_hsp: Option<bool>,
  pub mix_compile: Option<bool>,
  pub ignore_warning: Option<bool>,
  pub device_types: Option<Vec<String>>,
  pub resolve_module_paths: Option<Vec<String>>,
  #[napi(ts_type = "{ requestPermissions: unknown[]; definePermissions: unknown[] }")]
  pub permission: Option<serde_json::Value>,
  #[napi(ts_type = "Record<string, string>")]
  pub module_path_map: Option<serde_json::Value>,
  #[napi(ts_type = "Record<string, unknown>")]
  pub byte_code_har_info: Option<serde_json::Value>,
  #[napi(ts_type = "Record<string, string>")]
  pub hsp_name_ohm_map: Option<serde_json::Value>,
  #[napi(ts_type = "Record<string, string>")]
  pub har_name_ohm_map: Option<serde_json::Value>,
  pub other_compile_files: Option<Vec<String>>,
}

/// Compile plugin share object available as `this.share`.
///
/// The source and package indices stay in Rust. JavaScript receives this live
/// object once per module build, so `getHashByFilePath`,
/// `getCachedFileInfo`, and `importResolver` do not serialize a project-wide
/// source table or perform hook-time filesystem reads.
#[napi(object, object_from_js = false)]
pub struct HvigorShareContext {
  #[napi(js_name = "__arkdownBuildInstanceId", skip_typescript)]
  pub build_instance_id: String,
  #[napi(ts_type = "HvigorProjectConfig & Record<string, unknown>")]
  pub project_config: serde_json::Value,
  #[napi(js_name = "__arkdownNativeShare", skip_typescript)]
  pub native_share: External<HvigorNativeShareContext>,
  #[napi(js_name = "__arkdownPluginCacheSnapshot", skip_typescript)]
  pub plugin_cache_snapshot: Option<Buffer>,
  #[napi(ts_type = "Record<string, string[]>")]
  pub symlink_map: serde_json::Value,
  #[napi(ts_type = "{ enableIncre: boolean; changedDeps: string[] }")]
  pub dep_info: serde_json::Value,
  #[napi(ts_type = "Map<string, unknown>")]
  pub cache: Option<String>,
  #[napi(ts_type = "Map<string, unknown>")]
  pub common_cache: Option<String>,
  #[napi(
    ts_type = "{ keys(): IterableIterator<string>; unmount(key: string): void; mount(key: string): { getCache(cacheKey: string): unknown; setCache(cacheKey: string, value: unknown): void } }"
  )]
  pub cache_store_manager: Option<String>,
  #[napi(ts_type = "(path: string) => string")]
  pub get_hash_by_file_path: Option<String>,
  #[napi(ts_type = "(path: string, hash: string) => void")]
  pub set_hash_value_by_file_path: Option<String>,
  #[napi(ts_type = "(path: string) => HvigorCachedFileInfo | undefined")]
  pub get_cached_file_info: Option<String>,
  #[napi(ts_type = "() => string[]")]
  pub get_source_files: Option<String>,
  #[napi(ts_type = "(request: string) => HvigorImportResolution | undefined")]
  pub import_resolver: Option<String>,
  #[napi(ts_type = "(prefix: string) => unknown")]
  pub get_logger: Option<String>,
  #[napi(ts_type = "(prefix: string) => unknown")]
  pub get_hvigor_console_logger: Option<String>,
  #[napi(ts_type = "(pluginName: string, hookName: string) => HvigorHookEventFactory | undefined")]
  pub get_hook_event_factory: Option<String>,
  #[napi(ts_type = "(error: unknown) => never")]
  pub throw_ark_ts_compiler_error: Option<String>,
  #[napi(ts_type = "(id: string) => void")]
  pub record_error_file_id: Option<String>,
}

pub struct HvigorNativeShareContext {
  build_instance_id: u64,
  source_files: Arc<FxHashMap<PathBuf, HvigorCompilePluginSourceFile>>,
  import_resolver_entries: Arc<[HvigorCompilePluginImportEntry]>,
}

impl From<&HvigorCompilePluginShare> for HvigorShareContext {
  fn from(share: &HvigorCompilePluginShare) -> Self {
    Self {
      build_instance_id: share.build_instance_id.to_string(),
      project_config: serde_json::Value::Object(share.project_config.as_ref().clone()),
      native_share: External::new(HvigorNativeShareContext {
        build_instance_id: share.build_instance_id,
        source_files: Arc::clone(&share.source_files),
        import_resolver_entries: Arc::clone(&share.import_resolver_entries),
      }),
      plugin_cache_snapshot: share
        .plugin_cache_snapshot
        .as_ref()
        .map(|snapshot| Buffer::from(snapshot.to_vec())),
      symlink_map: serde_json::Value::Object(
        share
          .symlink_map
          .iter()
          .map(|(actual, symlinks)| {
            (
              arkdown_path::ArtifactPathSerializer::path_to_slash_string(actual)
                .unwrap_or_else(|_| actual.to_string_lossy().into_owned()),
              serde_json::Value::Array(
                symlinks
                  .iter()
                  .map(|symlink| {
                    serde_json::Value::String(
                      arkdown_path::ArtifactPathSerializer::path_to_slash_string(symlink)
                        .unwrap_or_else(|_| symlink.to_string_lossy().into_owned()),
                    )
                  })
                  .collect(),
              ),
            )
          })
          .collect(),
      ),
      dep_info: serde_json::json!({
        "enableIncre": share.dependency_info.enable_incremental,
        "changedDeps": share.dependency_info.changed_dependencies.as_ref(),
      }),
      cache: None,
      common_cache: None,
      cache_store_manager: None,
      get_hash_by_file_path: None,
      set_hash_value_by_file_path: None,
      get_cached_file_info: None,
      get_source_files: None,
      import_resolver: None,
      get_logger: None,
      get_hvigor_console_logger: None,
      get_hook_event_factory: None,
      throw_ark_ts_compiler_error: None,
      record_error_file_id: None,
    }
  }
}

#[napi(object)]
pub struct HvigorCachedFileInfo {
  pub exists: bool,
  pub is_file: bool,
  pub is_directory: bool,
  pub mtime_ms: Option<f64>,
  pub size: i64,
}

#[napi(object)]
pub struct HvigorImportResolution {
  pub normalized_path: String,
  pub package_name: String,
  pub package_path: String,
}

/// Function shape returned by a Hvigor hook-event factory. JavaScript creates
/// the callable compatibility object; every event operation is recorded by
/// the native observability pipeline.
#[napi(object, object_from_js = false)]
pub struct HvigorHookEvent {
  #[napi(ts_type = "(name: string) => HvigorHookEvent")]
  pub create_event: String,
  #[napi(ts_type = "(name: string) => HvigorHookEvent")]
  pub create_sub_event: String,
  #[napi(ts_type = "(time?: number) => HvigorHookEvent")]
  pub start: String,
  #[napi(ts_type = "(state?: string, time?: number) => void")]
  pub stop: String,
  #[napi(ts_type = "(time?: number) => HvigorHookEvent")]
  pub start_async_event: String,
  #[napi(ts_type = "(state?: string, time?: number) => void")]
  pub stop_async_event: String,
  #[napi(ts_type = "(time: number) => void")]
  pub set_total_time: String,
}

#[napi(object, object_from_js = false)]
pub struct HvigorHookEventFactory {
  #[napi(ts_type = "(name: string) => HvigorHookEvent")]
  pub create_event: String,
}

#[napi(object)]
pub struct HvigorHookEventRecord {
  pub event_id: String,
  pub parent_event_id: Option<String>,
  pub plugin_name: String,
  pub hook_name: String,
  pub event_name: String,
  pub operation: String,
  pub timestamp_ms: Option<f64>,
  pub state: Option<String>,
  pub total_time_ms: Option<f64>,
}

impl HvigorNativeShareContext {
  fn record_hook_event(&self, record: HvigorHookEventRecord) {
    tracing::info!(
      target: "arkdown_napi::metric",
      metric = "hvigor_compile_event",
      build_instance_id = self.build_instance_id,
      event_id = record.event_id,
      parent_event_id = record.parent_event_id.as_deref(),
      plugin_name = record.plugin_name,
      hook_name = record.hook_name,
      event_name = record.event_name,
      operation = record.operation,
      timestamp_ms = record.timestamp_ms,
      state = record.state.as_deref(),
      total_time_ms = record.total_time_ms,
      "compile plugin performance event"
    );
  }

  fn get_hash_by_file_path(&self, path: String) -> Option<String> {
    self
      .source_file(PathBuf::from(path).as_path())
      .map(|source| source.content_hash.clone())
  }

  fn get_cached_file_info(&self, path: String) -> Option<HvigorCachedFileInfo> {
    self
      .source_file(PathBuf::from(path).as_path())
      .map(|source| HvigorCachedFileInfo {
        exists: true,
        is_file: true,
        is_directory: false,
        mtime_ms: source.modified_millis.map(|value| value as f64),
        size: source.size.min(i64::MAX as u64) as i64,
      })
  }

  fn get_source_files(&self) -> napi::Result<Vec<String>> {
    let mut paths = self
      .source_files
      .keys()
      .map(|path| arkdown_path::ArtifactPathSerializer::path_to_slash_string(path))
      .collect::<std::result::Result<Vec<_>, _>>()
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    paths.sort_unstable();
    Ok(paths)
  }

  fn import_resolver(&self, request: String) -> napi::Result<Option<HvigorImportResolution>> {
    let normalized_request = request.replace('\\', "/");
    let normalized_request = normalized_request.trim_end_matches('/');
    let entry = self
      .import_resolver_entries
      .iter()
      .filter(|entry| {
        normalized_request == entry.alias
          || normalized_request
            .strip_prefix(entry.alias.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
      })
      .max_by_key(|entry| entry.alias.len());
    let Some(entry) = entry else {
      return Ok(None);
    };
    let suffix = normalized_request
      .strip_prefix(entry.alias.as_str())
      .unwrap_or_default()
      .trim_start_matches('/');
    let requested_path = if suffix.is_empty() {
      entry.entry_path.clone().unwrap_or_default()
    } else {
      PathBuf::from(suffix)
    };
    let resolved_path = self.resolve_package_source(&entry.package_root, &requested_path);
    let relative = resolved_path
      .strip_prefix(&entry.package_root)
      .unwrap_or(resolved_path.as_path());
    let normalized_relative = arkdown_path::ArtifactPathSerializer::path_to_slash_string(relative)
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    let normalized_path = if normalized_relative.is_empty() {
      entry.package_name.clone()
    } else {
      format!(
        "{}/{}",
        entry.package_name,
        Self::without_source_extension(&normalized_relative)
      )
    };
    Ok(Some(HvigorImportResolution {
      normalized_path,
      package_name: entry.package_name.clone(),
      package_path: arkdown_path::ArtifactPathSerializer::path_to_slash_string(&entry.package_root)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?,
    }))
  }

  fn source_file(&self, path: &std::path::Path) -> Option<&HvigorCompilePluginSourceFile> {
    let path = arkdown_path::lexical_normalize(path);
    self.source_files.get(&path).or_else(|| {
      let path_text = path.to_string_lossy();
      let alternate = if let Some(suffix) = path_text.strip_prefix("/private/") {
        PathBuf::from(format!("/{suffix}"))
      } else if path_text.starts_with("/var/") || path_text.starts_with("/tmp/") {
        PathBuf::from(format!("/private{path_text}"))
      } else {
        return None;
      };
      self.source_files.get(&alternate)
    })
  }

  fn resolve_package_source(
    &self,
    package_root: &std::path::Path,
    path: &std::path::Path,
  ) -> PathBuf {
    let candidate = arkdown_path::lexical_normalize(&package_root.join(path));
    if self.source_files.contains_key(&candidate) {
      return candidate;
    }
    let path_text = candidate.to_string_lossy();
    for extension in ["ets", "ts", "js", "mjs", "cjs", "d.ets", "d.ts"] {
      let with_extension = PathBuf::from(format!("{path_text}.{extension}"));
      if self.source_files.contains_key(&with_extension) {
        return with_extension;
      }
    }
    for file_name in ["Index.ets", "index.ets", "Index.ts", "index.ts", "index.js"] {
      let index = candidate.join(file_name);
      if self.source_files.contains_key(&index) {
        return index;
      }
    }
    candidate
  }

  fn without_source_extension(path: &str) -> &str {
    for extension in [".d.ets", ".d.ts", ".ets", ".ts", ".mjs", ".cjs", ".js"] {
      if let Some(path) = path.strip_suffix(extension) {
        return path;
      }
    }
    path
  }
}

#[napi(skip_typescript)]
pub fn hvigor_share_get_hash_by_file_path(
  share: &External<HvigorNativeShareContext>,
  path: String,
) -> Option<String> {
  share.get_hash_by_file_path(path)
}

#[napi(skip_typescript)]
pub fn hvigor_share_get_cached_file_info(
  share: &External<HvigorNativeShareContext>,
  path: String,
) -> Option<HvigorCachedFileInfo> {
  share.get_cached_file_info(path)
}

#[napi(skip_typescript)]
pub fn hvigor_share_get_source_files(
  share: &External<HvigorNativeShareContext>,
) -> napi::Result<Vec<String>> {
  share.get_source_files()
}

#[napi(skip_typescript)]
pub fn hvigor_share_import_resolver(
  share: &External<HvigorNativeShareContext>,
  request: String,
) -> napi::Result<Option<HvigorImportResolution>> {
  share.import_resolver(request)
}

/// Record one operation on a source-compatible Hvigor `CompileEvent`.
///
/// Event objects must live in V8 because TS/JS plugins call their methods
/// directly. The compatibility adapter sends each typed operation here, so
/// timing ownership and build-log output stay in Rust.
#[napi(skip_typescript)]
pub fn hvigor_share_record_hook_event(
  share: &External<HvigorNativeShareContext>,
  record: HvigorHookEventRecord,
) {
  share.record_hook_event(record);
}

/// Dependency info returned by `this.getModuleInfo(id)` inside plugin hooks.
#[napi(object)]
pub struct HvigorModuleInfo {
  pub id: String,
  pub code: Either<String, Null>,
  pub ast: Null,
  pub original_code: Either<String, Null>,
  pub imported_ids: Vec<String>,
  pub importers: Vec<String>,
  pub is_entry: bool,
  pub is_external: bool,
  pub is_included: Either<bool, Null>,
  pub has_default_export: Either<bool, Null>,
  pub exports: Either<Vec<String>, Null>,
  pub dynamically_imported_ids: Vec<String>,
  pub exported_bindings: Either<HashMap<String, Vec<String>>, Null>,
  pub dynamic_importers: Vec<String>,
  pub meta: serde_json::Value,
  #[napi(ts_type = "boolean | 'no-treeshake' | null")]
  pub module_side_effects: Option<JsHookSideEffects>,
  pub description: Option<String>,
  pub package_json_path: Option<String>,
  #[napi(ts_type = "'es' | 'cjs' | 'unknown'")]
  pub input_format: String,
}

impl HvigorModuleInfo {
  fn from_rolldown(module_info: &ModuleInfo) -> Self {
    let exports = module_info
      .exports
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>();
    Self {
      id: module_info.id.to_string(),
      code: module_info
        .code
        .as_ref()
        .map_or(Either::B(Null), |code| Either::A(code.to_string())),
      ast: Null,
      original_code: Either::B(Null),
      imported_ids: module_info
        .imported_ids
        .iter()
        .map(ToString::to_string)
        .collect(),
      importers: module_info
        .importers
        .iter()
        .map(ToString::to_string)
        .collect(),
      is_entry: module_info.is_entry,
      is_external: false,
      is_included: Either::B(Null),
      has_default_export: Either::A(exports.iter().any(|name| name == "default")),
      exports: Either::A(exports),
      dynamically_imported_ids: module_info
        .dynamically_imported_ids
        .iter()
        .map(ToString::to_string)
        .collect(),
      exported_bindings: Either::B(Null),
      dynamic_importers: module_info
        .dynamic_importers
        .iter()
        .map(ToString::to_string)
        .collect(),
      meta: serde_json::Value::Object(serde_json::Map::new()),
      module_side_effects: None,
      description: None,
      package_json_path: None,
      input_format: match module_info.input_format {
        rolldown_common::ExportsKind::Esm => "es",
        rolldown_common::ExportsKind::CommonJs => "cjs",
        rolldown_common::ExportsKind::None => "unknown",
      }
      .to_string(),
    }
  }
}

/// Compile plugin context — the `this` value inside user plugin hooks.
///
/// The plugin adapter binds this object before forwarding hooks into the Rust
/// NAPI bridge. Methods are typed here for generated TypeScript declarations;
/// the actual method implementations live in `js/plugin-adapter.js`.
#[napi(object)]
pub struct HvigorPluginContext {
  #[napi(ts_type = "HvigorShareContext")]
  pub share: Option<String>,
  #[napi(ts_type = "Map<string, unknown>")]
  pub cache: Option<String>,
  #[napi(ts_type = "{ rollupVersion: string; rolldownVersion: string; watchMode: boolean }")]
  pub meta: Option<String>,
  #[napi(
    ts_type = "(source: string, importer?: string, options?: JsPluginContextResolveOptions) => Promise<JsResolveIdOutput | null>"
  )]
  pub resolve: Option<String>,
  #[napi(ts_type = "(options: JsPluginContextLoadOptions) => Promise<HvigorModuleInfo | null>")]
  pub load: Option<String>,
  #[napi(ts_type = "(file: JsEmittedFile) => string")]
  pub emit_file: Option<String>,
  #[napi(ts_type = "(id: string) => void")]
  pub add_watch_file: Option<String>,
  #[napi(ts_type = "() => IterableIterator<string>")]
  pub get_module_ids: Option<String>,
  #[napi(ts_type = "(id: string) => HvigorModuleInfo | null")]
  pub get_module_info: Option<String>,
  #[napi(ts_type = "() => Record<string, unknown>")]
  pub get_combined_sourcemap: Option<String>,
  #[napi(ts_type = "(source: string, options?: unknown) => unknown")]
  pub parse: Option<String>,
  #[napi(ts_type = "(moduleRef: string) => string")]
  pub get_file_name: Option<String>,
  #[napi(ts_type = "(value: unknown) => void")]
  pub info: Option<String>,
  #[napi(ts_type = "(value: unknown) => void")]
  pub warn: Option<String>,
  #[napi(ts_type = "(value: unknown) => void")]
  pub debug: Option<String>,
  #[napi(ts_type = "(value: unknown) => never")]
  pub error: Option<String>,
  #[napi(ts_type = "(operation: (...args: any[]) => unknown, ...args: any[]) => unknown")]
  pub r#async: Option<String>,
  #[napi(ts_type = "() => void")]
  pub signal: Option<String>,
  #[napi(ts_type = "() => Promise<void>")]
  pub block: Option<String>,
}

/// Latest-hvigor cache invalidation input.
#[napi(object)]
pub struct PluginCacheRequest {
  #[napi(ts_type = "unknown")]
  pub ast: Option<String>,
  pub code: String,
  pub id: String,
}

/// Rolldown-compatible arkdown compile plugin.
///
/// The plugin adapter accepts objects passed explicitly to `build(config,
/// plugins)` or loaded from `compilePluginPath`/`hvigorPluginFile`. Hooks are
/// normalized by `js/plugin-adapter.js` before they are passed to Rust.
#[napi(object)]
pub struct ArkdownPlugin {
  pub name: Option<String>,
  #[napi(ts_type = "'first' | 'last' | ((nameToIndex: Record<string, number>) => number)")]
  pub order: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext) => void | Promise<void> }"
  )]
  pub before_build: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, options?: JsBuildStartOptions) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, options?: JsBuildStartOptions) => void | Promise<void> }"
  )]
  pub build_start: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext) => void | Promise<void> }"
  )]
  pub after_build_start: Option<String>,
  #[napi(
    ts_type = "((id: string, importer?: string | null, options?: JsResolveIdOptions) => string | JsResolveIdOutput | false | null | undefined | Promise<string | JsResolveIdOutput | false | null | undefined>) | { order?: 'pre' | 'post'; handler: (id: string, importer?: string | null, options?: JsResolveIdOptions) => string | JsResolveIdOutput | false | null | undefined | Promise<string | JsResolveIdOutput | false | null | undefined> }"
  )]
  pub resolve_id: Option<String>,
  #[napi(
    ts_type = "((id: string) => string | JsLoadOutput | null | undefined | Promise<string | JsLoadOutput | null | undefined>) | { order?: 'pre' | 'post'; handler: (id: string) => string | JsLoadOutput | null | undefined | Promise<string | JsLoadOutput | null | undefined> }"
  )]
  pub load: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, code: string, id: string, options: JsTransformHookExtraArgs) => string | JsTransformOutput | null | undefined | Promise<string | JsTransformOutput | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, code: string, id: string, options: JsTransformHookExtraArgs) => string | JsTransformOutput | null | undefined | Promise<string | JsTransformOutput | null | undefined> }"
  )]
  pub transform: Option<String>,
  #[napi(
    ts_type = "((moduleInfo: HvigorModuleInfo) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (moduleInfo: HvigorModuleInfo) => void | Promise<void> }"
  )]
  pub module_parsed: Option<String>,
  #[napi(
    ts_type = "((id: string, importer?: string | null, options?: JsResolveIdOptions) => string | JsResolveIdOutput | false | null | undefined | Promise<string | JsResolveIdOutput | false | null | undefined>) | { order?: 'pre' | 'post'; handler: (id: string, importer?: string | null, options?: JsResolveIdOptions) => string | JsResolveIdOutput | false | null | undefined | Promise<string | JsResolveIdOutput | false | null | undefined> }"
  )]
  pub resolve_dynamic_import: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, id: string, change: { event: 'create' | 'update' | 'delete' }) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, id: string, change: { event: 'create' | 'update' | 'delete' }) => void | Promise<void> }"
  )]
  pub watch_change: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, input?: unknown) => unknown) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, input?: unknown) => unknown }"
  )]
  pub before_build_end: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, error?: Error) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, error?: Error) => void | Promise<void> }"
  )]
  pub build_end: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext) => void | Promise<void> }"
  )]
  pub after_build_end: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext) => void | Promise<void> }"
  )]
  pub bundle_end: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext) => void | Promise<void> }"
  )]
  pub clean_up: Option<String>,
  #[napi(
    ts_type = "((request: PluginCacheRequest) => boolean | Promise<boolean>) | { order?: 'pre' | 'post'; handler: (request: PluginCacheRequest) => boolean | Promise<boolean> }"
  )]
  pub should_invalid_cache: Option<String>,
  /// Accepted for Rollup plugin source compatibility. Stage/ESMODULE ArkTS
  /// builds stop after module-graph collection, so output hooks remain dormant
  /// just as they do when ets2bundle enables `generateModuleGraphOnly` and
  /// `disableGenerateBundle`.
  #[napi(
    ts_type = "((this: HvigorPluginContext, options: Record<string, unknown>) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, options: Record<string, unknown>) => void | Promise<void> }"
  )]
  pub render_start: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined> }"
  )]
  pub banner: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined> }"
  )]
  pub footer: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined> }"
  )]
  pub intro: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined> }"
  )]
  pub outro: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, options: Record<string, unknown>) => unknown | Promise<unknown>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, options: Record<string, unknown>) => unknown | Promise<unknown> }"
  )]
  pub render_dynamic_import: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, options: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, options: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined> }"
  )]
  pub resolve_file_url: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, property: string | null, options: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, property: string | null, options: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined> }"
  )]
  pub resolve_import_meta: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, code: string, chunk: Record<string, unknown>, options: Record<string, unknown>) => string | { code: string; map?: unknown } | null | undefined | Promise<string | { code: string; map?: unknown } | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, code: string, chunk: Record<string, unknown>, options: Record<string, unknown>) => string | { code: string; map?: unknown } | null | undefined | Promise<string | { code: string; map?: unknown } | null | undefined> }"
  )]
  pub render_chunk: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, chunk: Record<string, unknown>) => string | null | undefined | Promise<string | null | undefined> }"
  )]
  pub augment_chunk_hash: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, error: Error) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, error: Error) => void | Promise<void> }"
  )]
  pub render_error: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, options: Record<string, unknown>, bundle: Record<string, unknown>, isWrite: boolean) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, options: Record<string, unknown>, bundle: Record<string, unknown>, isWrite: boolean) => void | Promise<void> }"
  )]
  pub generate_bundle: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext, options: Record<string, unknown>, bundle: Record<string, unknown>) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext, options: Record<string, unknown>, bundle: Record<string, unknown>) => void | Promise<void> }"
  )]
  pub write_bundle: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext) => void | Promise<void> }"
  )]
  pub close_bundle: Option<String>,
  #[napi(
    ts_type = "((this: HvigorPluginContext) => void | Promise<void>) | { order?: 'pre' | 'post'; handler: (this: HvigorPluginContext) => void | Promise<void> }"
  )]
  pub close_watcher: Option<String>,
}

#[napi(object)]
pub struct BuildConfig {
  pub project: Option<String>,
  pub target: Option<String>,
  pub mode: Option<String>,
  /// Product action. Omitted performs a complete assemble.
  #[napi(
    ts_type = "'assemble' | 'hot-reload-compile' | 'hot-reload-build' | 'cold-reload' | 'prepare-quickfix' | 'package-quickfix' | 'sign-quickfix' | 'assemble-quickfix' | 'device-test'"
  )]
  pub action: Option<String>,
  /// Hvigor-compatible performance analysis mode. Hook event factories are
  /// active only for verbose and trace.
  #[napi(ts_type = "'normal' | 'default' | 'verbose' | 'false' | 'trace'")]
  pub analyze: Option<String>,
  /// Explicit ArkTS compiler language override. Omitted uses the selected
  /// product's `arkTSVersion`, defaulting to ArkTS 1.1.
  #[napi(ts_type = "'1.1' | '1.2'")]
  pub arkts_version: Option<String>,
  pub module: Option<String>,
  pub product: Option<String>,
  pub build_target_name: Option<String>,
  pub build_version: Option<String>,
  pub watch: Option<bool>,
  pub preview: Option<bool>,
  pub check_entry: Option<bool>,
  pub coverage: Option<bool>,
  pub scope: Option<String>,
  pub ohos_test_coverage: Option<bool>,
  pub coverage_path_filter: Option<String>,
  #[napi(ts_type = "'black' | 'white' | 'full'")]
  pub coverage_mode: Option<String>,
  pub unit_test_replace_page: Option<String>,
  pub ohos_debug_asan: Option<bool>,
  pub ohos_enable_hwasan: Option<bool>,
  pub ohos_enable_tsan: Option<bool>,
  pub ohos_enable_ubsan: Option<bool>,
  pub ohos_enable_binxo: Option<bool>,
  /// Compile native C/C++ sources with `zig cc` and `zig c++`.
  pub enable_zig: Option<bool>,
  pub disable_abc_cache: Option<bool>,
  pub cache_file: Option<String>,
  /// Enable the source compiler's ArkUI-X cross-platform API contract.
  pub is_crossplatform: Option<bool>,
  /// Match `ignoreCrossplatformCheck`: emit 11706007 as a warning instead of an error.
  pub ignore_crossplatform_check: Option<bool>,
  /// Skip build diagnostics while retaining checker facts required by source transforms.
  pub skip_checker: Option<bool>,
  pub cache: Option<bool>,
  /// Node executable of the JavaScript process that owns this build. The JS
  /// service wrapper injects `process.execPath`; SDK subprocesses must not
  /// silently select a different Node version from PATH.
  pub node_executable: Option<String>,
  #[napi(ts_type = "string | string[]")]
  pub compile_plugin_path: Option<String>,
  #[napi(ts_type = "string | string[]")]
  pub hvigor_plugin_file: Option<String>,
}

#[napi(object)]
pub struct BuildResult {
  pub artifacts: Vec<String>,
  pub elapsed_ms: f64,
}

// ---------------------------------------------------------------------------
// build() — the main NAPI export
// ---------------------------------------------------------------------------

/// Build an OHOS project from Node.js.
///
/// Accepts a `BuildConfig` and an optional array of Rolldown-compatible
/// plugin objects. Plugins are called in the same Node.js process via
/// NAPI ThreadsafeFunction — no subprocess, no IPC overhead.
///
/// @example
/// ```typescript
/// import { build } from '@ohos-rs/arkdown'
///
/// const result = await build({
///   project: './my-app',
///   target: 'hap',
///   mode: 'debug',
///   plugins: [{
///     name: 'env-replace',
///     transform(code, id) {
///       return code.replace(/process\.env\.NODE_ENV/g, '"production"')
///     },
///   }],
/// })
/// process.stdout.write(`${result.artifacts.join('\n')}\n`)
/// ```
/// Build an OHOS project from Node.js.
///
/// Returns a Promise that resolves with the build result.
/// Plugins are wrapped synchronously before spawning the async build task,
/// so non-Send JS values (Env, Array) don't cross the tokio task boundary.
#[napi(skip_typescript)]
pub fn build<'a>(
  env: Env,
  config: BuildConfig,
  #[napi(ts_arg_type = "Array<ArkdownPlugin> | undefined | null")] plugins: Option<Array<'a>>,
  sdk_plugin_dispatch: Function<'a, (String,), String>,
) -> napi::Result<Object<'a>> {
  runtime_observe::RuntimeObservability::install_build_safety_net(&config)
    .map_err(|error| napi::Error::from_reason(error.to_string()))?;
  let sdk_plugin_host = Some(NapiSdkPluginHost::from_dispatch(sdk_plugin_dispatch)?);

  // 1. Wrap JS plugins synchronously (Env + Array are not Send).
  let mut js_plugins: Vec<PackagePlugin> = Vec::new();
  if let Some(arr) = plugins {
    let len = arr.len();
    for i in 0..len {
      let obj = arr.get::<Object<'_>>(i)?.ok_or_else(|| {
        napi::Error::from_reason(format!(
          "ARKDOWN_PLUGIN_EXPORT_INVALID: plugin[{i}] is missing"
        ))
      })?;
      let (plugin, order) = JsPlugin::from_js_object(obj).map_err(|error| {
        napi::Error::from_reason(format!(
          "ARKDOWN_PLUGIN_EXPORT_INVALID: plugin[{i}]: {}",
          error.reason
        ))
      })?;
      let module_root = plugin.module_root.clone();
      let target_name = plugin.target_name.clone();
      let rolldown_plugin = plugin.clone() as rolldown_plugin::__inner::SharedPluginable;
      let gap_hooks = plugin as Arc<dyn HvigorPluginGapHooks>;
      js_plugins.push(
        PackagePlugin::with_gap_hooks(rolldown_plugin, order, gap_hooks)
          .with_module_root(module_root)
          .with_target_name(target_name),
      );
    }
  }

  // Extract all config values (Send types only).
  let BuildConfig {
    project,
    target,
    mode,
    action,
    analyze,
    arkts_version,
    module,
    product,
    build_target_name,
    build_version,
    watch,
    preview,
    check_entry,
    coverage,
    scope,
    ohos_test_coverage,
    coverage_path_filter,
    coverage_mode,
    unit_test_replace_page,
    ohos_debug_asan,
    ohos_enable_hwasan,
    ohos_enable_tsan,
    ohos_enable_ubsan,
    ohos_enable_binxo,
    enable_zig,
    disable_abc_cache,
    cache_file,
    is_crossplatform,
    ignore_crossplatform_check,
    skip_checker,
    cache,
    node_executable,
    compile_plugin_path: _,
    hvigor_plugin_file: _,
  } = config;
  let project = project.unwrap_or_else(|| ".".to_string());
  let target = target.unwrap_or_else(|| "hap".to_string());
  let mode = mode.unwrap_or_else(|| "debug".to_string());
  let package_args = PackageBuildArgs {
    action: arkdown_build::request::BuildAction::from_name(
      action.as_deref().unwrap_or("assemble"),
    )
    .ok_or_else(|| {
      napi::Error::from_reason(format!(
        "invalid build action '{}'; expected assemble, hot-reload-compile, hot-reload-build, cold-reload, prepare-quickfix, package-quickfix, sign-quickfix, assemble-quickfix, or device-test",
        action.as_deref().unwrap_or("assemble")
      ))
    })?,
    arkts_language_version: arkts_version
      .as_deref()
      .map(str::parse)
      .transpose()
      .map_err(|error: arkdown_core::ParseArktsLanguageVersionError| {
        napi::Error::from_reason(error.to_string())
      })?,
    analyze: arkdown_build::request::AnalyzeModeChoice::from_name(
      analyze.as_deref().unwrap_or("normal"),
    )
    .ok_or_else(|| {
      napi::Error::from_reason(format!(
        "invalid analyze mode '{}'; expected normal, verbose, false, or trace",
        analyze.as_deref().unwrap_or("normal")
      ))
    })?,
    build_target_name,
    build_version,
    watch: watch.unwrap_or(false),
    preview: preview.unwrap_or(false),
    check_entry: check_entry.unwrap_or(false),
    coverage: coverage.unwrap_or(false),
    scope,
    ohos_test_coverage,
    coverage_path_filter,
    coverage_mode,
    unit_test_replace_page,
    ohos_debug_asan: ohos_debug_asan.unwrap_or(false),
    ohos_enable_hwasan: ohos_enable_hwasan.unwrap_or(false),
    ohos_enable_tsan: ohos_enable_tsan.unwrap_or(false),
    ohos_enable_ubsan: ohos_enable_ubsan.unwrap_or(false),
    ohos_enable_binxo: ohos_enable_binxo.unwrap_or(false),
    enable_zig: enable_zig.unwrap_or(false),
    disable_abc_cache: disable_abc_cache.unwrap_or(false),
    cache_file,
    is_crossplatform: is_crossplatform.unwrap_or(false),
    ignore_crossplatform_check: ignore_crossplatform_check.unwrap_or(false),
    skip_checker: skip_checker.unwrap_or(false),
    cache: cache.unwrap_or(true),
    node_executable: node_executable.map(std::path::PathBuf::from),

    sdk_plugin_host,
    heap_reclaimer: Some(heap_reclaimer::NapiHeapReclaimer::from_env(&env)?),
  };

  // 2. Spawn the async build and return a JS Promise.
  // execute_tokio_future handles the deferred Promise pattern for tokio_rt.
  let raw_promise = napi::bindgen_prelude::execute_tokio_future(
    env.raw(),
    async move {
      let start = std::time::Instant::now();
      let artifacts = run_build(
        project,
        target,
        mode,
        module,
        product,
        package_args,
        js_plugins,
      )
      .await
      .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;
      let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
      Ok::<_, napi::Error>(BuildResult {
        artifacts,
        elapsed_ms,
      })
    },
    |env_raw, result| unsafe { napi::bindgen_prelude::ToNapiValue::to_napi_value(env_raw, result) },
  )?;
  unsafe { Object::from_napi_value(env.raw(), raw_promise) }
}

/// Parse JavaScript for plugin glue that is not currently inside a live
/// Rolldown callback.
#[napi]
pub fn parse_plugin_ast(
  source: String,
  options: Option<JsPluginAstParseOptions>,
) -> napi::Result<serde_json::Value> {
  ArkdownPluginAstBridge::parse(&source, options)
}

#[derive(Debug, Clone, Default)]
struct PackageBuildArgs {
  action: arkdown_build::request::BuildAction,
  arkts_language_version: Option<arkdown_core::ArktsLanguageVersion>,
  analyze: arkdown_build::request::AnalyzeModeChoice,
  build_target_name: Option<String>,
  build_version: Option<String>,
  watch: bool,
  preview: bool,
  check_entry: bool,
  coverage: bool,
  scope: Option<String>,
  ohos_test_coverage: Option<bool>,
  coverage_path_filter: Option<String>,
  coverage_mode: Option<String>,
  unit_test_replace_page: Option<String>,
  ohos_debug_asan: bool,
  ohos_enable_hwasan: bool,
  ohos_enable_tsan: bool,
  ohos_enable_ubsan: bool,
  ohos_enable_binxo: bool,
  enable_zig: bool,
  disable_abc_cache: bool,
  cache_file: Option<String>,
  is_crossplatform: bool,
  ignore_crossplatform_check: bool,
  skip_checker: bool,
  cache: bool,
  node_executable: Option<std::path::PathBuf>,
  sdk_plugin_host: Option<Arc<dyn NativeOhSdkPluginHost>>,
  heap_reclaimer: Option<Arc<dyn arkdown_build::request::BuildHeapReclaimer>>,
}

async fn run_build(
  project: String,
  target: String,
  mode: String,
  module: Option<String>,
  product: Option<String>,
  package_args: PackageBuildArgs,
  js_plugins: Vec<PackagePlugin>,
) -> anyhow::Result<Vec<String>> {
  use arkdown_build::request::AbcBackendChoice;
  use arkdown_build::request::BuildRequest;
  let project_path = std::path::PathBuf::from(&project);
  let args = BuildRequest {
    action: package_args.action,
    target,
    mode,
    analyze: package_args.analyze,
    arkts_version: package_args.arkts_language_version,
    module,
    product,
    project: Some(project_path.clone()),
    build_target_name: package_args.build_target_name,
    build_version: package_args.build_version,
    watch: package_args.watch,
    coverage: package_args.coverage,
    scope: package_args.scope,
    ohos_test_coverage: package_args.ohos_test_coverage,
    coverage_path_filter: package_args.coverage_path_filter,
    coverage_mode: package_args.coverage_mode,
    unit_test_replace_page: package_args.unit_test_replace_page,
    fast_build_app: false,
    enable_sign_task: true,
    ohos_debug_asan: package_args.ohos_debug_asan,
    ohos_enable_hwasan: package_args.ohos_enable_hwasan,
    ohos_enable_tsan: package_args.ohos_enable_tsan,
    ohos_enable_ubsan: package_args.ohos_enable_ubsan,
    ohos_enable_binxo: package_args.ohos_enable_binxo,
    enable_zig: package_args.enable_zig,
    preview: package_args.preview,
    check_entry: package_args.check_entry,
    preview_transform_mode: false,
    disable_abc_cache: package_args.disable_abc_cache || !package_args.cache,
    cache_file: package_args.cache_file.map(std::path::PathBuf::from),
    abc_backend: AbcBackendChoice::Sdk,
    is_crossplatform: package_args.is_crossplatform,
    ignore_crossplatform_check: package_args.ignore_crossplatform_check,
    skip_checker: package_args.skip_checker,
    cache: package_args.cache,
    // Hot-rebuild flags are always false when invoked via the Node.js API
    // — only the watch stage sets them for incremental rebuilds.
    skip_resource: false,
    skip_profile: false,
    watch_invalidation: None,
    pack_compress_level: None,
    align_device_types: Vec::new(),
    collect_debug_symbol: None,
    ui_transform_optimization: false,
    integrated_hsp_use_get_current_bundle_name: false,
    byte_code_har_integrated_optimization: false,
    keep_dependency: true,
    hvigor_properties: Default::default(),
    parameter_file_override: None,
    node_executable: package_args.node_executable,
    sdk_plugin_host: package_args.sdk_plugin_host,
    heap_reclaimer: package_args.heap_reclaimer,
    hvigor_cache_dir: Some(project_path.join(".hvigor")),
  };

  let output = arkdown_build::BuildService::new(args)
    .with_package_plugins(js_plugins)
    .execute()
    .await?;

  Ok(
    output
      .artifact_paths
      .iter()
      .map(|p| arkdown_path::ArtifactPathSerializer::path_to_slash_string(p))
      .collect::<std::result::Result<Vec<_>, _>>()?,
  )
}
