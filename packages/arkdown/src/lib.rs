//! arkdown Node.js bindings (napi-rs 3.9.x).
//!
//! Exposes a `build()` function that accepts Rolldown-compatible JS plugin
//! objects. Plugin hooks are called via NAPI `ThreadsafeFunction` — same
//! process, same V8 heap, no subprocess overhead.

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

mod build_script_runtime_model;
pub mod cli_plan;
pub mod cmd;
mod hvigor_config_merge;
mod hvigor_json;
mod hvigor_maintenance;
mod hvigor_plugin_plan;
mod hvigor_script;
mod hvigor_sync;
mod native_build_config;
pub mod progress;
mod runtime_task;
pub mod task_cache;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction};
use napi_derive::napi;
use tracing_subscriber::EnvFilter;

use rolldown_common::{
  side_effects::HookSideEffects, ModuleInfo, ModuleType, NormalModule, ResolvedExternal,
};
use rolldown_plugin::{
  HookBuildEndArgs, HookBuildStartArgs, HookLoadArgs, HookLoadOutput, HookLoadReturn,
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookTransformArgs,
  HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage, Plugin,
  PluginContext, SharedLoadPluginContext, SharedTransformPluginContext,
};

use arkdown::plugins::factory::{HvigorPluginGapHooks, HvigorPluginOrder, PackagePlugin};
use arkdown::plugins::hvigor_compile::HvigorNativePluginSlot;
use arkdown::PluginCacheModule;

/// Parse the latest-hvigor command surface in Rust. JavaScript only revives
/// the typed plan and dispatches the requested runtime operation.
#[napi]
pub fn parse_hvigor_cli_args(args: Vec<String>) -> napi::Result<String> {
  cli_plan::PackageCliParser::parse_hvigor_json(&args).map_err(|error| {
    napi::Error::from_reason(format!("failed to serialize hvigor CLI plan: {error}"))
  })
}

/// Parse Arkdown's direct `build` command arguments in Rust.
#[napi]
pub fn parse_arkdown_build_cli_args(args: Vec<String>) -> napi::Result<String> {
  cli_plan::PackageCliParser::parse_build_json(&args).map_err(|error| {
    napi::Error::from_reason(format!(
      "failed to serialize Arkdown build CLI plan: {error}"
    ))
  })
}

/// Normalize the public/runtime build request in Rust while preserving live
/// JavaScript extension values that the plugin glue may still reference.
#[napi]
pub fn prepare_native_build_config<'env>(
  env: Env,
  config: Object<'env>,
  overlay: Option<native_build_config::RuntimeBuildConfigOverlay>,
  process_executable: String,
) -> napi::Result<Object<'env>> {
  native_build_config::NativeBuildConfigNapiAdapter::new(env, config, process_executable)
    .prepare(overlay)
}

/// Deep-merge callback-produced hvigor configuration in Rust without
/// serializing live JavaScript functions or object identities.
#[napi]
pub fn merge_hvigor_config<'env>(
  env: Env,
  base: Unknown<'env>,
  overlay: Unknown<'env>,
) -> napi::Result<Unknown<'env>> {
  hvigor_config_merge::HvigorConfigMergeNapiAdapter::new(env).merge(base, overlay)
}

/// Compute latest-hvigor's normalized 32-character project cache identity in
/// the Rust session layer.
#[napi]
pub fn hvigor_project_cache_hash(project_root: String) -> napi::Result<String> {
  arkdown_session::BuildScriptProjectCacheIdentity::new(project_root)
    .map(|identity| identity.hash())
    .map_err(|error| napi::Error::from_reason(error.to_string()))
}

/// Load the immutable project and module model files once in Rust. JavaScript
/// consumes the resulting snapshot only to construct plugin-facing facades.
#[napi]
pub fn load_build_script_runtime_model(request_json: String) -> napi::Result<String> {
  build_script_runtime_model::BuildScriptRuntimeModelNapiAdapter::load_project(&request_json)
}

/// Load one module added dynamically by `hvigorconfig.ts::includeNode`.
#[napi]
pub fn load_build_script_module_model(request_json: String) -> napi::Result<String> {
  build_script_runtime_model::BuildScriptRuntimeModelNapiAdapter::load_module(&request_json)
}

/// Select one hvigorfile per root using the latest installed hvigor suffix
/// precedence without repeating filesystem probes in JavaScript.
#[napi]
pub fn discover_build_script_files(request_json: String) -> napi::Result<String> {
  build_script_runtime_model::BuildScriptRuntimeModelNapiAdapter::discover_files(&request_json)
}

/// Resolve target selection, effective build options, and compile-plugin
/// records in Rust after JavaScript configuration callbacks have completed.
#[napi]
pub fn plan_build_script_runtime(request_json: String) -> napi::Result<String> {
  build_script_runtime_model::BuildScriptRuntimeModelNapiAdapter::plan(&request_json)
}

/// Validate callback-produced build options with the same Rust schema owner
/// used for project and module profile snapshots.
#[napi]
pub fn validate_build_script_build_option(request_json: String) -> napi::Result<String> {
  build_script_runtime_model::BuildScriptRuntimeModelNapiAdapter::validate_build_option(
    &request_json,
  )
}

/// Collect and resolve plugin paths from a callback-produced hvigorfile value.
#[napi]
pub fn collect_build_script_plugin_paths(
  value: Unknown<'_>,
  base_dir: String,
  include_compile_plugin_file: Option<bool>,
) -> napi::Result<String> {
  build_script_runtime_model::BuildScriptPluginPathNapiAdapter::collect(
    value,
    base_dir,
    include_compile_plugin_file.unwrap_or(true),
  )
}

/// Resolve one configured plugin path with latest-hvigor extension order.
#[napi]
pub fn resolve_build_script_plugin_path(candidate: String, base_dir: String) -> Option<String> {
  build_script_runtime_model::BuildScriptPluginPathNapiAdapter::resolve(candidate, base_dir)
}

/// Parse hvigor's JSON5-shaped public utility values through Rust/OXC. The
/// tagged JSON bridge lets JavaScript revive values JSON cannot represent.
#[napi]
pub fn parse_hvigor_json_text(source: String, locations: Option<bool>) -> napi::Result<String> {
  hvigor_json::HvigorJsonNapiParser::parse(&source, locations.unwrap_or(false))
}

/// Compile one hvigor TypeScript configuration module through Rust OXC and
/// Rolldown. JavaScript receives only the CommonJS text that it must execute
/// in the owning V8 isolate.
#[napi]
pub async fn compile_hvigor_script(file: String, source_map: Option<bool>) -> napi::Result<String> {
  hvigor_script::HvigorScriptNapiCompiler::compile(file, source_map.unwrap_or(false)).await
}

/// Synchronous counterpart required by Node's `Module._extensions` contract
/// for TypeScript files imported from another hvigor configuration module.
#[napi]
pub fn compile_hvigor_script_sync(file: String, source_map: Option<bool>) -> napi::Result<String> {
  hvigor_script::HvigorScriptNapiCompiler::compile_sync(file, source_map.unwrap_or(false))
}

/// Resolve latest-hvigor compile-plugin order against the native Rolldown
/// pipeline. JavaScript functions are invoked synchronously on the owning V8
/// thread; all grouping, built-in inventory, validation, and slot reduction
/// remain in Rust.
#[napi]
pub fn plan_hvigor_compile_plugins<'env>(
  env: Env,
  config_json: String,
  plugins: Array<'env>,
) -> napi::Result<String> {
  hvigor_plugin_plan::HvigorCompilePluginNapiPlanner::from_js(env, &config_json, plugins)?
    .plan_json()
}

/// Resolve the function-valued order extension for explicit package plugins.
/// String-valued first/last orders stay on the plugin for Rolldown's Rust
/// factory; only callback execution crosses into JavaScript.
#[napi]
pub fn plan_rolldown_package_plugins<'env>(env: Env, plugins: Array<'env>) -> napi::Result<String> {
  hvigor_plugin_plan::RolldownPackagePluginNapiPlanner::from_js(env, plugins)?.plan_json()
}

/// Validate raw compile-plugin exports before JavaScript wraps their function
/// identities into Rolldown hook adapters.
#[napi]
pub fn validate_hvigor_compile_plugins(
  plugins: Array<'_>,
  plugin_file: String,
) -> napi::Result<String> {
  hvigor_plugin_plan::HvigorPluginNapiValidator::validate_compile(plugins, plugin_file)
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
}

#[napi(object)]
pub struct JsResolveIdOutput {
  pub id: String,
  pub external: Option<JsResolvedExternal>,
  pub normalize_external_id: Option<bool>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<JsHookSideEffects>,
  pub package_json_path: Option<String>,
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
}

impl From<PluginContext> for ArkdownRolldownPluginContext {
  fn from(inner: PluginContext) -> Self {
    Self { inner }
  }
}

struct ArkdownPluginAstBridge;

impl ArkdownPluginAstBridge {
  fn parse(source: &str, options_json: Option<&str>) -> napi::Result<String> {
    let options = arkdown::PluginAstParseOptions::from_json(options_json)
      .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    arkdown::PluginAstParser::new(source, options)
      .parse()
      .and_then(|result| result.bridge_json())
      .map_err(|error| napi::Error::from_reason(error.to_string()))
  }
}

#[napi]
impl ArkdownRolldownPluginContext {
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
  pub fn parse(&self, source: String, options_json: Option<String>) -> napi::Result<String> {
    ArkdownPluginAstBridge::parse(&source, options_json.as_deref())
  }
}

type ResolveIdTsfn = Arc<
  ThreadsafeFunction<
    (
      ArkdownRolldownPluginContext,
      String,
      Option<String>,
      JsResolveIdOptions,
    ),
    Option<Either3<String, JsResolveIdOutput, bool>>,
    FnArgs<(
      ArkdownRolldownPluginContext,
      String,
      Option<String>,
      JsResolveIdOptions,
    )>,
    napi::Status,
    false,
  >,
>;
type LoadTsfn = Arc<
  ThreadsafeFunction<
    (ArkdownRolldownPluginContext, String),
    Option<Either<String, JsLoadOutput>>,
    FnArgs<(ArkdownRolldownPluginContext, String)>,
    napi::Status,
    false,
  >,
>;
type TransformTsfn = Arc<
  ThreadsafeFunction<
    (ArkdownRolldownPluginContext, String, String),
    Option<Either<String, JsTransformOutput>>,
    FnArgs<(ArkdownRolldownPluginContext, String, String)>,
    napi::Status,
    false,
  >,
>;
type ContextNoopTsfn = Arc<
  ThreadsafeFunction<
    (ArkdownRolldownPluginContext, String),
    (),
    FnArgs<(ArkdownRolldownPluginContext, String)>,
    napi::Status,
    false,
  >,
>;
type JsonNoopTsfn = Arc<ThreadsafeFunction<String, (), FnArgs<(String,)>, napi::Status, false>>;
type BoolTsfn = Arc<ThreadsafeFunction<String, bool, FnArgs<(String,)>, napi::Status, false>>;

// ---------------------------------------------------------------------------
// JsPlugin — bridges a JavaScript plugin object to rolldown_plugin::Plugin
// ---------------------------------------------------------------------------

/// Wraps a JavaScript plugin object.
///
/// Each supported hook is an `Arc<ThreadsafeFunction<Input, Output>>` that
/// can be called from any Rust thread (including `tokio` async tasks).
/// This is the same bridge pattern used by rolldown_binding to connect
/// JS plugins to Rust's rolldown core.
pub struct JsPlugin {
  name: String,
  is_hvigor_compile_plugin: bool,
  module_root: Option<PathBuf>,
  target_name: Option<String>,
  // ── Rolldown bundler hooks ───────────────────────────────────────────────
  resolve_id_fn: Option<ResolveIdTsfn>,
  load_fn: Option<LoadTsfn>,
  transform_fn: Option<TransformTsfn>,
  build_start_fn: Option<ContextNoopTsfn>,
  module_parsed_fn: Option<ContextNoopTsfn>,
  before_build_end_fn: Option<ContextNoopTsfn>,
  build_end_fn: Option<ContextNoopTsfn>,
  /// `resolveDynamicImport` — same TSFN shape as `resolveId`.
  ///
  /// Corresponds to hvigor's `resolveDynamicImport` hook listed in
  /// `doWrapper` event names.
  resolve_dynamic_import_fn: Option<ResolveIdTsfn>,
  // ── Hvigor-only gaps around Rolldown ────────────────────────────────────
  should_invalid_cache_fn: Option<BoolTsfn>,
  /// Internal data seeding for hvigor's `this.share`; not a user hook.
  seed_hvigor_context_fn: Option<JsonNoopTsfn>,
  usage: HookUsage,
}

impl std::fmt::Debug for JsPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("JsPlugin")
      .field("name", &self.name)
      .finish_non_exhaustive()
  }
}

impl JsPlugin {
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

  fn module_info_value(module_info: &ModuleInfo) -> serde_json::Value {
    let exports = module_info
      .exports
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>();
    serde_json::json!({
      "id": module_info.id.to_string(),
      "code": module_info.code.as_ref().map(ToString::to_string),
      "ast": serde_json::Value::Null,
      "originalCode": serde_json::Value::Null,
      "importedIds": module_info
        .imported_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>(),
      "importers": module_info
        .importers
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>(),
      "isEntry": module_info.is_entry,
      "isExternal": false,
      "isIncluded": serde_json::Value::Null,
      "hasDefaultExport": exports.iter().any(|name| name == "default"),
      "exports": exports,
      "dynamicallyImportedIds": module_info
        .dynamically_imported_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>(),
      "exportedBindings": serde_json::Value::Null,
      "dynamicImporters": module_info
        .dynamic_importers
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>(),
    })
  }

  fn build_start_options_value(args: &HookBuildStartArgs<'_>) -> serde_json::Value {
    let input = if args.options.input.len() == 1 && args.options.input[0].name.is_none() {
      serde_json::Value::String(args.options.input[0].import.clone())
    } else if args.options.input.iter().all(|item| item.name.is_some()) {
      serde_json::Value::Object(
        args
          .options
          .input
          .iter()
          .filter_map(|item| {
            item
              .name
              .as_ref()
              .map(|name| (name.clone(), serde_json::Value::String(item.import.clone())))
          })
          .collect(),
      )
    } else {
      serde_json::Value::Array(
        args
          .options
          .input
          .iter()
          .map(|item| serde_json::Value::String(item.import.clone()))
          .collect(),
      )
    };
    serde_json::json!({
      "input": input,
      "cwd": args.options.cwd,
      "platform": args.options.platform.to_string(),
      "shimMissingExports": args.options.shim_missing_exports,
      "context": args.options.context,
    })
  }

  fn from_js_object(obj: Object<'_>) -> napi::Result<(Arc<Self>, HvigorPluginOrder)> {
    let name: String = obj
      .get::<String>("name")?
      .unwrap_or_else(|| "unknown".to_string());
    let is_hvigor_compile_plugin = obj
      .get::<bool>("__arkdownHvigorCompilePlugin")?
      .unwrap_or(false);
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

    let mut usage = HookUsage::empty();

    let build_start_fn = if let Some(js_fn) =
      obj.get::<Function<(ArkdownRolldownPluginContext, String), ()>>("buildStart")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(ArkdownRolldownPluginContext, String)>()
        .build_callback(
          |ctx: ThreadsafeCallContext<(ArkdownRolldownPluginContext, String)>| {
            Ok(FnArgs { data: ctx.value })
          },
        )?;
      usage |= HookUsage::BuildStart;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let resolve_id_fn = if let Some(js_fn) = obj.get::<Function<
      (
        ArkdownRolldownPluginContext,
        String,
        Option<String>,
        JsResolveIdOptions,
      ),
      Option<Either3<String, JsResolveIdOutput, bool>>,
    >>("resolveId")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(
          ArkdownRolldownPluginContext,
          String,
          Option<String>,
          JsResolveIdOptions,
        )>()
        .build_callback(
          |ctx: ThreadsafeCallContext<(
            ArkdownRolldownPluginContext,
            String,
            Option<String>,
            JsResolveIdOptions,
          )>| { Ok(FnArgs { data: ctx.value }) },
        )?;
      usage |= HookUsage::ResolveId;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let load_fn = if let Some(js_fn) = obj.get::<Function<
      (ArkdownRolldownPluginContext, String),
      Option<Either<String, JsLoadOutput>>,
    >>("load")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(ArkdownRolldownPluginContext, String)>()
        .build_callback(
          |ctx: ThreadsafeCallContext<(ArkdownRolldownPluginContext, String)>| {
            Ok(FnArgs { data: ctx.value })
          },
        )?;
      usage |= HookUsage::Load;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let transform_fn = if let Some(js_fn) = obj.get::<Function<
      (ArkdownRolldownPluginContext, String, String),
      Option<Either<String, JsTransformOutput>>,
    >>("transform")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(ArkdownRolldownPluginContext, String, String)>()
        .build_callback(
          |ctx: ThreadsafeCallContext<(ArkdownRolldownPluginContext, String, String)>| {
            Ok(FnArgs { data: ctx.value })
          },
        )?;
      usage |= HookUsage::Transform;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let module_parsed_fn = if let Some(js_fn) =
      obj.get::<Function<(ArkdownRolldownPluginContext, String), ()>>("moduleParsed")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(ArkdownRolldownPluginContext, String)>()
        .build_callback(
          |ctx: ThreadsafeCallContext<(ArkdownRolldownPluginContext, String)>| {
            Ok(FnArgs { data: ctx.value })
          },
        )?;
      usage |= HookUsage::ModuleParsed;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let before_build_end_fn = if let Some(js_fn) =
      obj.get::<Function<(ArkdownRolldownPluginContext, String), ()>>("beforeBuildEnd")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(ArkdownRolldownPluginContext, String)>()
        .build_callback(
          |ctx: ThreadsafeCallContext<(ArkdownRolldownPluginContext, String)>| {
            Ok(FnArgs { data: ctx.value })
          },
        )?;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let build_end_fn = if let Some(js_fn) =
      obj.get::<Function<(ArkdownRolldownPluginContext, String), ()>>("buildEnd")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(ArkdownRolldownPluginContext, String)>()
        .build_callback(
          |ctx: ThreadsafeCallContext<(ArkdownRolldownPluginContext, String)>| {
            Ok(FnArgs { data: ctx.value })
          },
        )?;
      usage |= HookUsage::BuildEnd;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let should_invalid_cache_fn =
      if let Some(js_fn) = obj.get::<Function<(String,), bool>>("shouldInvalidCache")? {
        let tsfn = js_fn
          .build_threadsafe_function::<String>()
          .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
        Some(Arc::new(tsfn))
      } else {
        None
      };

    // ── resolveDynamicImport — same TSFN shape as resolveId ─────────────────
    let resolve_dynamic_import_fn = if let Some(js_fn) = obj.get::<Function<
      (
        ArkdownRolldownPluginContext,
        String,
        Option<String>,
        JsResolveIdOptions,
      ),
      Option<Either3<String, JsResolveIdOutput, bool>>,
    >>("resolveDynamicImport")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(
          ArkdownRolldownPluginContext,
          String,
          Option<String>,
          JsResolveIdOptions,
        )>()
        .build_callback(
          |ctx: ThreadsafeCallContext<(
            ArkdownRolldownPluginContext,
            String,
            Option<String>,
            JsResolveIdOptions,
          )>| { Ok(FnArgs { data: ctx.value }) },
        )?;
      usage |= HookUsage::ResolveDynamicImport;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let seed_hvigor_context_fn =
      if let Some(js_fn) = obj.get::<Function<(String,), ()>>("__arkdownSeedHvigorContext")? {
        let tsfn = js_fn
          .build_threadsafe_function::<String>()
          .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
        Some(Arc::new(tsfn))
      } else {
        None
      };

    tracing::debug!(
      plugin = %name,
      module_root = ?module_root,
      target_name = ?target_name,
      "wrapped JS plugin"
    );
    Ok((
      Arc::new(Self {
        name,
        is_hvigor_compile_plugin,
        module_root,
        target_name,
        resolve_id_fn,
        load_fn,
        transform_fn,
        build_start_fn,
        module_parsed_fn,
        before_build_end_fn,
        build_end_fn,
        resolve_dynamic_import_fn,
        should_invalid_cache_fn,
        seed_hvigor_context_fn,
        usage,
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
    let options_json = if self.is_hvigor_compile_plugin {
      String::new()
    } else {
      Self::build_start_options_value(args).to_string()
    };
    let plugin_context = ctx.clone().into();
    let tsfn = self.build_start_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(()) };
      f.call_async((plugin_context, options_json))
        .await
        .map_err(|e| anyhow::anyhow!("buildStart JS error: {e}"))?;
      Ok(())
    }
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
    };
    let plugin_context = ctx.clone().into();
    let tsfn = self.resolve_id_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(None) };
      match f
        .call_async((plugin_context, specifier.clone(), importer, options))
        .await
      {
        Ok(Some(output)) => Ok(Some(Self::resolve_output(output, specifier)?)),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("resolveId JS error: {e}")),
      }
    }
  }

  /// `resolveDynamicImport` — bridges dynamic import resolution to JS.
  ///
  /// Same TSFN shape as `resolveId`.  Corresponds to the hook listed in
  /// hvigor's `doWrapper`/`updateEventNames` event name list.
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
    };
    let plugin_context = ctx.clone().into();
    let tsfn = self.resolve_dynamic_import_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(None) };
      match f
        .call_async((plugin_context, specifier.clone(), importer, options))
        .await
      {
        Ok(Some(output)) => Ok(Some(Self::resolve_output(output, specifier)?)),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("resolveDynamicImport JS error: {e}")),
      }
    }
  }

  fn load(
    &self,
    ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    let id = args.id.to_string();
    let plugin_context = ctx.inner.clone().into();
    let tsfn = self.load_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(None) };
      match f.call_async((plugin_context, id)).await {
        Ok(Some(Either::A(code))) => Ok(Some(HookLoadOutput {
          code: code.into(),
          ..Default::default()
        })),
        Ok(Some(Either::B(output))) => Ok(Some(output.try_into()?)),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("load JS error: {e}")),
      }
    }
  }

  fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
    let code = args.code.to_string();
    let id = args.id.to_string();
    let plugin_context = ctx.inner.clone().into();
    let tsfn = self.transform_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(None) };
      match f.call_async((plugin_context, code, id)).await {
        Ok(Some(Either::A(new_code))) => Ok(Some(HookTransformOutput {
          code: Some(new_code),
          ..Default::default()
        })),
        Ok(Some(Either::B(output))) => Ok(Some(output.try_into()?)),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("transform JS error: {e}")),
      }
    }
  }

  fn module_parsed(
    &self,
    ctx: &PluginContext,
    module_info: Arc<ModuleInfo>,
    _normal_module: &NormalModule,
  ) -> impl std::future::Future<Output = rolldown_plugin::HookNoopReturn> + Send {
    let module_info_json = Self::module_info_value(&module_info).to_string();
    let plugin_context = ctx.clone().into();
    let tsfn = self.module_parsed_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(()) };
      f.call_async((plugin_context, module_info_json))
        .await
        .map_err(|e| anyhow::anyhow!("moduleParsed JS error: {e}"))?;
      Ok(())
    }
  }

  fn build_end(
    &self,
    ctx: &PluginContext,
    args: Option<&HookBuildEndArgs<'_>>,
  ) -> impl std::future::Future<Output = rolldown_plugin::HookNoopReturn> + Send {
    // Keep Rolldown's native context alive through the JavaScript callback.
    // This matches the latest hvigor wrapper and avoids cloning every module's
    // source into an intermediate JSON graph for each compile plugin.
    //
    // # Upstream correspondence
    //
    // Corresponds to hvigor's `invalid-cache-plugin.ts::buildEnd`:
    //   for (const t of this.getModuleIds()) {
    //     const a = this.getModuleInfo(t);
    //     e.set(t, a ? a.importedIds.concat(a.dynamicallyImportedIds) : []);
    //   }
    // The build-end JSON carries only error metadata; module graph and emitted
    // file queries delegate directly to Rolldown through the hidden context.
    let should_call = args.is_none() || !self.is_hvigor_compile_plugin;
    let errors = args.map_or_else(Vec::new, |args| {
      args
        .errors
        .iter()
        .map(|error| {
          serde_json::json!({
            "message": error.to_string(),
            "id": error.id(),
            "plugin": error.plugin(),
          })
        })
        .collect::<Vec<_>>()
    });
    let build_end_json = should_call.then(|| {
      serde_json::to_string(&serde_json::json!({
        "errorCount": errors.len(),
        "errors": errors,
      }))
      .unwrap_or_default()
    });
    let plugin_context = should_call.then(|| ArkdownRolldownPluginContext::from(ctx.clone()));

    let tsfn = self.build_end_fn.clone();
    async move {
      // DevEco Studio 6.24.3 skips compile-plugin buildEnd when graph.build()
      // throws. Explicit Rolldown plugins retain the standard buildEnd(error)
      // callback; only strict hvigor compile plugins suppress that path.
      let (Some(plugin_context), Some(build_end_json)) = (plugin_context, build_end_json) else {
        return Ok(());
      };
      let Some(f) = tsfn else { return Ok(()) };
      f.call_async((plugin_context, build_end_json))
        .await
        .map_err(|e| anyhow::anyhow!("buildEnd JS error: {e}"))?;
      Ok(())
    }
  }
}

#[async_trait::async_trait]
impl HvigorPluginGapHooks for JsPlugin {
  async fn seed_hvigor_context(&self, share_json: &str) -> anyhow::Result<()> {
    let Some(f) = self.seed_hvigor_context_fn.clone() else {
      return Ok(());
    };
    f.call_async(share_json.to_string())
      .await
      .map_err(|e| anyhow::anyhow!("seed hvigor compile context JS error: {e}"))?;
    Ok(())
  }

  async fn should_invalid_cache(&self, module: &PluginCacheModule) -> anyhow::Result<bool> {
    let Some(f) = self.should_invalid_cache_fn.clone() else {
      return Ok(false);
    };
    // @ohos/hvigor-arkts-compose 6.24.3 ShouldInvalidCacheOptions is exactly
    // `{ ast?: AcornNode, code: string, id: string }`. OXC constructs the AST
    // here in Rust; JavaScript only revives JSON-inexpressible literal values.
    let request_json = module.hook_request_json()?;
    f.call_async(request_json)
      .await
      .map_err(|e| anyhow::anyhow!("shouldInvalidCache JS error: {e}"))
  }

  fn has_should_invalid_cache(&self) -> bool {
    self.should_invalid_cache_fn.is_some()
  }

  async fn before_build_end_with_context(&self, context: PluginContext) -> anyhow::Result<()> {
    let Some(f) = self.before_build_end_fn.clone() else {
      return Ok(());
    };
    f.call_async((context.into(), String::new()))
      .await
      .map_err(|e| anyhow::anyhow!("beforeBuildEnd JS error: {e}"))?;
    Ok(())
  }

  fn has_before_build_end(&self) -> bool {
    self.before_build_end_fn.is_some()
  }

  fn runtime_cache_identity(&self) -> Option<String> {
    let mut hooks = Vec::new();
    // Rolldown hooks
    if self.build_start_fn.is_some() {
      hooks.push("buildStart");
    }
    if self.resolve_id_fn.is_some() {
      hooks.push("resolveId");
    }
    if self.load_fn.is_some() {
      hooks.push("load");
    }
    if self.transform_fn.is_some() {
      hooks.push("transform");
    }
    if self.module_parsed_fn.is_some() {
      hooks.push("moduleParsed");
    }
    if self.resolve_dynamic_import_fn.is_some() {
      hooks.push("resolveDynamicImport");
    }
    if self.before_build_end_fn.is_some() {
      hooks.push("beforeBuildEnd");
    }
    if self.build_end_fn.is_some() {
      hooks.push("buildEnd");
    }
    // Hvigor-only hooks
    if self.should_invalid_cache_fn.is_some() {
      hooks.push("shouldInvalidCache");
    }
    (!hooks.is_empty()).then(|| hooks.join(","))
  }
}

// ---------------------------------------------------------------------------
// NAPI-exposed config and result types
// ---------------------------------------------------------------------------

/// Project config metadata available as `this.share.projectConfig` inside plugin hooks.
///
/// This corresponds to the subset of hvigor's `projectConfig` that arkdown
/// serializes from the Rust build coordinator and passes through `runtime.js`.
#[napi(object)]
pub struct HvigorProjectConfig {
  pub bundle_name: Option<String>,
  pub build_mode: Option<String>,
  pub target: Option<String>,
  #[napi(js_name = "useNormalizedOHMUrl")]
  pub use_normalized_ohm_url: Option<bool>,
  pub compile_sdk_version: Option<String>,
  pub compatible_sdk_version: Option<String>,
  pub package_manager_type: Option<String>,
  pub is_byte_code_har_optimize: Option<bool>,
}

/// Compile plugin share object available as `this.share`.
#[napi(object)]
pub struct HvigorShareContext {
  pub project_config: Option<HvigorProjectConfig>,
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
    }
  }
}

/// Compile plugin context — the `this` value inside user plugin hooks.
///
/// Runtime JS binds this object before forwarding hooks into the Rust NAPI
/// bridge. Methods are typed here for generated TypeScript declarations; the
/// actual method implementations live in `runtime.js`.
#[napi(object)]
pub struct HvigorPluginContext {
  pub share: Option<HvigorShareContext>,
  #[napi(ts_type = "() => IterableIterator<string>")]
  pub get_module_ids: Option<String>,
  #[napi(ts_type = "(id: string) => HvigorModuleInfo | null")]
  pub get_module_info: Option<String>,
  #[napi(ts_type = "(source: string, options?: unknown) => unknown")]
  pub parse: Option<String>,
  #[napi(ts_type = "(moduleRef: string) => string")]
  pub get_file_name: Option<String>,
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
/// The runtime loader accepts plugin objects from explicit `build(config,
/// plugins)`, `compilePluginPath`, `hvigorPluginFile`, and discovered
/// hvigorfiles. Hooks are normalized by `runtime.js` before they are passed to
/// Rust, so user-facing cache hooks receive structured objects while the native
/// bridge still exchanges JSON strings internally.
#[napi(object)]
pub struct ArkdownPlugin {
  pub name: Option<String>,
  #[napi(ts_type = "'first' | 'last' | ((nameToIndex: Record<string, number>) => number)")]
  pub order: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext) => void | Promise<void>")]
  pub build_start: Option<String>,
  #[napi(
    ts_type = "(id: string, importer?: string | null, options?: JsResolveIdOptions) => string | JsResolveIdOutput | false | null | undefined | Promise<string | JsResolveIdOutput | false | null | undefined>"
  )]
  pub resolve_id: Option<String>,
  #[napi(
    ts_type = "(id: string) => string | JsLoadOutput | null | undefined | Promise<string | JsLoadOutput | null | undefined>"
  )]
  pub load: Option<String>,
  #[napi(
    ts_type = "(code: string, id: string) => string | JsTransformOutput | null | undefined | Promise<string | JsTransformOutput | null | undefined>"
  )]
  pub transform: Option<String>,
  #[napi(ts_type = "(moduleInfo: HvigorModuleInfo) => void | Promise<void>")]
  pub module_parsed: Option<String>,
  #[napi(
    ts_type = "(id: string, importer?: string | null, options?: JsResolveIdOptions) => string | JsResolveIdOutput | false | null | undefined | Promise<string | JsResolveIdOutput | false | null | undefined>"
  )]
  pub resolve_dynamic_import: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, input?: unknown) => unknown")]
  pub before_build_end: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, error?: Error) => void | Promise<void>")]
  pub build_end: Option<String>,
  #[napi(ts_type = "(request: PluginCacheRequest) => boolean | Promise<boolean>")]
  pub should_invalid_cache: Option<String>,
}

#[napi(object)]
pub struct BuildConfig {
  pub project: Option<String>,
  pub target: Option<String>,
  pub mode: Option<String>,
  pub module: Option<String>,
  pub product: Option<String>,
  pub build_target_name: Option<String>,
  pub build_version: Option<String>,
  pub watch: Option<bool>,
  pub preview: Option<bool>,
  pub coverage: Option<bool>,
  pub ohos_debug_asan: Option<bool>,
  pub ohos_enable_hwasan: Option<bool>,
  pub disable_abc_cache: Option<bool>,
  pub cache_file: Option<String>,
  pub checker: Option<String>,
  #[napi(ts_type = "'oxc' | 'yuku'")]
  pub module_analyzer: Option<String>,
  pub cache: Option<bool>,
  #[napi(ts_type = "string | string[]")]
  pub compile_plugin_path: Option<String>,
  #[napi(ts_type = "string | string[]")]
  pub hvigor_plugin_file: Option<String>,
  /// Runtime-owned serialized `project/hvigorfile.*` OHOS overrides.
  #[napi(skip_typescript)]
  pub project_hvigor_overrides_json: Option<String>,
  /// Runtime-owned serialized module-root build-option overrides.
  #[napi(skip_typescript)]
  pub module_hvigor_overrides_json: Option<String>,
  /// Runtime-owned complete model values replaced through OHOS contexts.
  #[napi(skip_typescript)]
  pub runtime_context_overrides_json: Option<String>,
  /// Runtime-owned custom TaskInput paths for the Rust watch inventory.
  #[napi(skip_typescript)]
  pub runtime_task_inputs_json: Option<String>,
  /// Runtime-owned numeric mapping of hvigor's `ohos.pack.compressLevel`.
  #[napi(skip_typescript)]
  pub pack_compress_level: Option<u32>,
  /// Runtime-owned hvigor `ohos.align.deviceTypes` list.
  #[napi(skip_typescript)]
  pub align_device_types: Option<Vec<String>>,
  /// Runtime-owned hvigor `ohos.collect.debugSymbol` override.
  #[napi(skip_typescript)]
  pub collect_debug_symbol: Option<bool>,
  /// Runtime-owned hvigor `ohos.uiTransform.Optimization` property.
  #[napi(skip_typescript)]
  pub ui_transform_optimization: Option<bool>,
  /// Runtime-owned hvigor `ohos.integratedHsp.useGetCurrentBundleName` property.
  #[napi(skip_typescript)]
  pub integrated_hsp_use_get_current_bundle_name: Option<bool>,
  /// Runtime-owned hvigor `ohos.byteCodeHar.integratedOptimization` property.
  #[napi(skip_typescript)]
  pub byte_code_har_integrated_optimization: Option<bool>,
  /// Runtime-owned hvigor `hvigor.keepDependency` property.
  #[napi(skip_typescript)]
  pub keep_dependency: Option<bool>,
  /// Runtime-owned hvigor CLI `parameterFile` extra-parameter override.
  #[napi(skip_typescript)]
  pub parameter_file_override: Option<String>,
  /// Runtime-owned path to the Node executable hosting this native addon.
  #[napi(skip_typescript)]
  pub runtime_node_executable: Option<String>,
  /// Runtime-owned selected partial hvigor task roots.
  #[napi(skip_typescript)]
  pub native_task_roots_json: Option<String>,
}

#[napi(object)]
pub struct BuildResult {
  pub artifacts: Vec<String>,
  pub elapsed_ms: f64,
}

/// Internal input used by synchronous OHOS context getters in `runtime.js`.
#[napi(object)]
pub struct HvigorContextInspectConfig {
  pub project: String,
  pub product: Option<String>,
  #[napi(skip_typescript)]
  pub runtime_context_overrides_json: Option<String>,
}

/// Inspect the Rust-owned dependency and module-name projections exposed by
/// `OhosProjectContext` and `OhosModuleContext`.
#[napi]
pub fn inspect_hvigor_context(config: HvigorContextInspectConfig) -> napi::Result<String> {
  let runtime_overrides = config
    .runtime_context_overrides_json
    .as_deref()
    .map(serde_json::from_str::<arkdown_config::RuntimeContextOverrides>)
    .transpose()
    .map_err(|error| {
      napi::Error::from_reason(format!("invalid runtime context overrides: {error}"))
    })?
    .unwrap_or_default();
  let snapshot = arkdown_session::HvigorContextInspector::new(
    config.project,
    config.product.unwrap_or_else(|| "default".to_string()),
    std::sync::Arc::new(runtime_overrides),
  )
  .inspect()
  .map_err(|error| napi::Error::from_reason(format!("inspect hvigor context: {error}")))?;
  serde_json::to_string(&snapshot)
    .map_err(|error| napi::Error::from_reason(format!("serialize hvigor context: {error}")))
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
#[napi(ts_return_type = "Promise<BuildResult>")]
pub fn build<'a>(
  env: Env,
  config: BuildConfig,
  #[napi(ts_arg_type = "Array<ArkdownPlugin> | undefined | null")] plugins: Option<Array<'a>>,
  #[napi(
    ts_arg_type = "{ tasksJson: string; selectionJson: string; invoke: (operationJson: string) => string | Promise<string> } | undefined | null"
  )]
  runtime_tasks: Option<Object<'a>>,
) -> napi::Result<Object<'a>> {
  init_napi_tracing_from_env();

  // 1. Wrap JS plugins synchronously (Env + Array are not Send).
  let mut js_plugins: Vec<PackagePlugin> = Vec::new();
  if let Some(arr) = plugins {
    let len = arr.len();
    for i in 0..len {
      if let Some(obj) = arr.get::<Object<'_>>(i)? {
        match JsPlugin::from_js_object(obj) {
          Ok((p, order)) => {
            let module_root = p.module_root.clone();
            let target_name = p.target_name.clone();
            let plugin = p.clone() as rolldown_plugin::__inner::SharedPluginable;
            let gap_hooks = p as Arc<dyn HvigorPluginGapHooks>;
            js_plugins.push(
              PackagePlugin::with_gap_hooks(plugin, order, gap_hooks)
                .with_module_root(module_root)
                .with_target_name(target_name),
            );
          }
          Err(e) => tracing::warn!(index = i, error = %e, "failed to wrap plugin[{i}]"),
        }
      }
    }
  }

  let runtime_task_seed = runtime_tasks
    .map(crate::runtime_task::RuntimeTaskBridgeSeed::from_js_object)
    .transpose()?;
  let bridge_input_paths_json = runtime_task_seed
    .as_ref()
    .map(crate::runtime_task::RuntimeTaskBridgeSeed::declared_input_paths_json)
    .transpose()
    .map_err(|error| napi::Error::from_reason(format!("{error:#}")))?;

  // Extract all config values (Send types only).
  let BuildConfig {
    project,
    target,
    mode,
    module,
    product,
    build_target_name,
    build_version,
    watch,
    preview,
    coverage,
    ohos_debug_asan,
    ohos_enable_hwasan,
    disable_abc_cache,
    cache_file,
    checker,
    module_analyzer,
    cache,
    compile_plugin_path: _,
    hvigor_plugin_file: _,
    project_hvigor_overrides_json,
    module_hvigor_overrides_json,
    runtime_context_overrides_json,
    runtime_task_inputs_json: configured_runtime_task_inputs_json,
    pack_compress_level,
    align_device_types,
    collect_debug_symbol,
    ui_transform_optimization,
    integrated_hsp_use_get_current_bundle_name,
    byte_code_har_integrated_optimization,
    keep_dependency,
    parameter_file_override,
    runtime_node_executable,
    native_task_roots_json,
  } = config;
  let project = project.unwrap_or_else(|| ".".to_string());
  let target = target.unwrap_or_else(|| "hap".to_string());
  let mode = mode.unwrap_or_else(|| "debug".to_string());
  let package_args = PackageBuildArgs {
    build_target_name,
    build_version,
    watch: watch.unwrap_or(false),
    preview: preview.unwrap_or(false),
    coverage: coverage.unwrap_or(false),
    ohos_debug_asan: ohos_debug_asan.unwrap_or(false),
    ohos_enable_hwasan: ohos_enable_hwasan.unwrap_or(false),
    disable_abc_cache: disable_abc_cache.unwrap_or(false),
    cache_file,
    checker,
    module_analyzer,
    cache: cache.unwrap_or(true),
    project_hvigor_overrides_json,
    module_hvigor_overrides_json,
    runtime_context_overrides_json,
    runtime_task_inputs_json: bridge_input_paths_json.or(configured_runtime_task_inputs_json),
    pack_compress_level,
    align_device_types,
    collect_debug_symbol,
    ui_transform_optimization: ui_transform_optimization.unwrap_or(false),
    integrated_hsp_use_get_current_bundle_name: integrated_hsp_use_get_current_bundle_name
      .unwrap_or(false),
    byte_code_har_integrated_optimization: byte_code_har_integrated_optimization.unwrap_or(false),
    keep_dependency: keep_dependency.unwrap_or(true),
    parameter_file_override,
    runtime_node_executable,
    native_task_roots_json,
  };

  // 2. Spawn the async build and return a JS Promise.
  // execute_tokio_future handles the deferred Promise pattern for tokio_rt.
  let raw_promise = napi::bindgen_prelude::execute_tokio_future(
    env.raw(),
    async move {
      let start = std::time::Instant::now();
      let task_observer = match runtime_task_seed {
        Some(seed) => Some(
          crate::runtime_task::RuntimeTaskActorObserver::spawn(seed)
            .map_err(|error| napi::Error::from_reason(format!("{error:#}")))?,
        ),
        None => None,
      };
      let artifacts = run_build(
        project,
        target,
        mode,
        module,
        product,
        package_args,
        js_plugins,
        task_observer,
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

/// Execute custom hvigor command roots through the Rust-owned task graph.
/// JavaScript remains only the callback host for user-authored task actions.
#[napi(ts_return_type = "Promise<Array<string>>")]
pub fn run_runtime_tasks<'a>(
  env: Env,
  #[napi(
    ts_arg_type = "{ tasksJson: string; selectionJson: string; invoke: (operationJson: string) => string | Promise<string> }"
  )]
  runtime_tasks: Object<'a>,
) -> napi::Result<Object<'a>> {
  let seed = crate::runtime_task::RuntimeTaskBridgeSeed::from_js_object(runtime_tasks)?;
  let raw_promise = napi::bindgen_prelude::execute_tokio_future(
    env.raw(),
    async move {
      seed
        .execute()
        .await
        .map_err(|error| napi::Error::from_reason(format!("{error:#}")))
    },
    |env_raw, result| unsafe { napi::bindgen_prelude::ToNapiValue::to_napi_value(env_raw, result) },
  )?;
  unsafe { Object::from_napi_value(env.raw(), raw_promise) }
}

/// Return the latest-hvigor built-in task catalog from the Rust-owned graph
/// metadata. JavaScript wraps these records only to expose lazy plugin APIs.
#[napi]
pub fn runtime_task_catalog(request_json: String) -> napi::Result<String> {
  crate::runtime_task::RuntimeTaskCatalog::from_json(&request_json)
    .and_then(crate::runtime_task::RuntimeTaskCatalog::entries_json)
    .map_err(|error| {
      napi::Error::from_reason(format!("invalid runtime task catalog request: {error:#}"))
    })
}

/// Render latest-hvigor `tasks` and `taskTree` reports from the Rust-owned
/// task inventory. JavaScript only supplies plugin-authored task metadata.
#[napi]
pub fn runtime_task_report(request_json: String) -> napi::Result<String> {
  crate::runtime_task::RuntimeTaskReportRenderer::from_json(&request_json)
    .and_then(|renderer| renderer.render())
    .map_err(|error| napi::Error::from_reason(format!("invalid runtime task report: {error:#}")))
}

/// Resolve latest-hvigor command modes, task roots, custom-task native
/// dependencies, and native build request groups in the Rust task graph.
#[napi]
pub fn runtime_command_plan(request_json: String) -> napi::Result<String> {
  crate::runtime_task::RuntimeCommandPlanner::response_json(&request_json).map_err(|error| {
    napi::Error::from_reason(format!("invalid runtime command plan request: {error:#}"))
  })
}

/// Execute latest-hvigor project/module clean behavior in Rust.
#[napi(ts_return_type = "Promise<string>")]
pub fn run_hvigor_clean<'a>(env: Env, request_json: String) -> napi::Result<Object<'a>> {
  let runner =
    crate::hvigor_maintenance::HvigorCleanRunner::from_json(&request_json).map_err(|error| {
      napi::Error::from_reason(format!("invalid hvigor clean request: {error:#}"))
    })?;
  let raw_promise = napi::bindgen_prelude::execute_tokio_future(
    env.raw(),
    async move {
      runner
        .response_json()
        .await
        .map_err(|error| napi::Error::from_reason(format!("{error:#}")))
    },
    |env_raw, result| unsafe { napi::bindgen_prelude::ToNapiValue::to_napi_value(env_raw, result) },
  )?;
  unsafe { Object::from_napi_value(env.raw(), raw_promise) }
}

/// Execute latest-hvigor cache and pnpm store pruning in Rust.
#[napi(ts_return_type = "Promise<string>")]
pub fn run_hvigor_prune<'a>(env: Env, request_json: String) -> napi::Result<Object<'a>> {
  let runner =
    crate::hvigor_maintenance::HvigorPruneRunner::from_json(&request_json).map_err(|error| {
      napi::Error::from_reason(format!("invalid hvigor prune request: {error:#}"))
    })?;
  let raw_promise = napi::bindgen_prelude::execute_tokio_future(
    env.raw(),
    async move {
      runner
        .response_json()
        .await
        .map_err(|error| napi::Error::from_reason(format!("{error:#}")))
    },
    |env_raw, result| unsafe { napi::bindgen_prelude::ToNapiValue::to_napi_value(env_raw, result) },
  )?;
  unsafe { Object::from_napi_value(env.raw(), raw_promise) }
}

/// Compare latest-hvigor and SDK cache metadata without routing filesystem
/// behavior through the JavaScript plugin layer.
#[napi]
pub fn hvigor_sync_version_changed(request_json: String) -> napi::Result<bool> {
  crate::hvigor_sync::HvigorCacheState::from_json(&request_json)
    .map(|state| state.version_changed())
    .map_err(|error| napi::Error::from_reason(format!("{error:#}")))
}

/// Generate and write the latest IDE tooling model entirely in Rust.
#[napi]
pub async fn write_hvigor_sync_model(request_json: String) -> napi::Result<()> {
  let writer = crate::hvigor_sync::HvigorSyncModelWriter::from_json(&request_json)
    .map_err(|error| napi::Error::from_reason(format!("{error:#}")))?;
  writer
    .write()
    .await
    .map_err(|error| napi::Error::from_reason(format!("{error:#}")))
}

/// Persist the metadata used by latest-hvigor's version-change clean edge.
#[napi]
pub async fn write_hvigor_build_meta(request_json: String) -> napi::Result<()> {
  let state = crate::hvigor_sync::HvigorCacheState::from_json(&request_json)
    .map_err(|error| napi::Error::from_reason(format!("{error:#}")))?;
  state
    .write_build_meta()
    .await
    .map_err(|error| napi::Error::from_reason(format!("{error:#}")))
}

/// Parse JavaScript for plugin glue that is not currently inside a live
/// Rolldown callback (for example direct context construction in a hvigorfile).
#[napi]
pub fn parse_plugin_ast(source: String, options_json: Option<String>) -> napi::Result<String> {
  ArkdownPluginAstBridge::parse(&source, options_json.as_deref())
}

fn init_napi_tracing_from_env() {
  if std::env::var_os("ARKDOWN_LOG").is_none() && std::env::var_os("RUST_LOG").is_none() {
    return;
  }
  let filter = EnvFilter::try_from_env("ARKDOWN_LOG")
    .or_else(|_| EnvFilter::try_from_default_env())
    .unwrap_or_else(|_| EnvFilter::new("info"));
  let _ = tracing_subscriber::fmt()
    .with_env_filter(filter)
    .with_level(false)
    .with_target(false)
    .without_time()
    .with_writer(std::io::stderr)
    .try_init();
}

#[derive(Debug, Clone, Default)]
struct PackageBuildArgs {
  build_target_name: Option<String>,
  build_version: Option<String>,
  watch: bool,
  preview: bool,
  coverage: bool,
  ohos_debug_asan: bool,
  ohos_enable_hwasan: bool,
  disable_abc_cache: bool,
  cache_file: Option<String>,
  checker: Option<String>,
  module_analyzer: Option<String>,
  cache: bool,
  project_hvigor_overrides_json: Option<String>,
  module_hvigor_overrides_json: Option<String>,
  runtime_context_overrides_json: Option<String>,
  runtime_task_inputs_json: Option<String>,
  pack_compress_level: Option<u32>,
  align_device_types: Option<Vec<String>>,
  collect_debug_symbol: Option<bool>,
  ui_transform_optimization: bool,
  integrated_hsp_use_get_current_bundle_name: bool,
  byte_code_har_integrated_optimization: bool,
  keep_dependency: bool,
  parameter_file_override: Option<String>,
  runtime_node_executable: Option<String>,
  native_task_roots_json: Option<String>,
}

async fn run_build(
  project: String,
  target: String,
  mode: String,
  module: Option<String>,
  product: Option<String>,
  package_args: PackageBuildArgs,
  js_plugins: Vec<PackagePlugin>,
  task_observer: Option<Arc<dyn crate::cmd::build::coordinator::BuildTaskObserver>>,
) -> anyhow::Result<Vec<String>> {
  use crate::cmd::build::args::{AbcBackendChoice, ModuleAnalyzerChoice};
  use crate::cmd::build::BuildArgs;
  use arkdown_config::{
    ArkdownConfigLoader, ModuleHvigorOverrides, ProjectHvigorOverrides, RuntimeContextOverrides,
  };
  use arkdown_core::{BuildMode, OutputTarget};
  use arkdown_session::BuildSessionBuilder;

  if let Some(checker) = package_args.checker.as_deref() {
    if checker != "oxc" && checker != "off" {
      anyhow::bail!("unsupported checker '{checker}'; expected 'oxc' or 'off'");
    }
  }
  let module_analyzer = match package_args.module_analyzer.as_deref().unwrap_or("oxc") {
    "oxc" => ModuleAnalyzerChoice::Oxc,
    "yuku" => ModuleAnalyzerChoice::Yuku,
    other => anyhow::bail!("unsupported module analyzer '{other}'; expected 'oxc' or 'yuku'"),
  };

  let project_path = std::path::PathBuf::from(&project);
  let is_app_build = target == "app";
  let output_target = match target.as_str() {
    "hap" => OutputTarget::Hap,
    "hsp" => OutputTarget::Hsp,
    "har" => OutputTarget::Har,
    // APP is a project-level aggregate. The startup snapshot uses HAP as its
    // neutral module target; AppBuildCoordinator derives HAP/HSP child configs
    // from this one immutable session.
    "app" => OutputTarget::Hap,
    other => {
      anyhow::bail!("unsupported build target '{other}'; expected 'hap', 'hsp', 'har', or 'app'")
    }
  };
  let build_mode = match mode.as_str() {
    "debug" => BuildMode::Debug,
    "release" => BuildMode::Release,
    other => anyhow::bail!("unsupported build mode '{other}'; expected 'debug' or 'release'"),
  };

  let project_hvigor_overrides = package_args
    .project_hvigor_overrides_json
    .as_deref()
    .map(serde_json::from_str::<ProjectHvigorOverrides>)
    .transpose()
    .map_err(|error| anyhow::anyhow!("invalid project hvigor overrides: {error}"))?;
  let module_hvigor_overrides = package_args
    .module_hvigor_overrides_json
    .as_deref()
    .map(serde_json::from_str::<ModuleHvigorOverrides>)
    .transpose()
    .map_err(|error| anyhow::anyhow!("invalid module hvigor overrides: {error}"))?;
  let runtime_context_overrides = package_args
    .runtime_context_overrides_json
    .as_deref()
    .map(serde_json::from_str::<RuntimeContextOverrides>)
    .transpose()
    .map_err(|error| anyhow::anyhow!("invalid runtime context overrides: {error}"))?
    .map(Arc::new);
  let custom_task_input_paths = package_args
    .runtime_task_inputs_json
    .as_deref()
    .map(serde_json::from_str::<Vec<String>>)
    .transpose()
    .map_err(|error| anyhow::anyhow!("invalid custom task input paths: {error}"))?
    .unwrap_or_default()
    .into_iter()
    .map(PathBuf::from)
    .collect::<Vec<_>>();
  let native_task_roots = package_args
    .native_task_roots_json
    .as_deref()
    .map(serde_json::from_str::<Vec<crate::cmd::build::args::NativeTaskRoot>>)
    .transpose()
    .map_err(|error| anyhow::anyhow!("invalid native task roots: {error}"))?
    .unwrap_or_default();

  let config_loader = ArkdownConfigLoader::new(
    &project_path,
    build_mode,
    output_target,
    product.as_deref(),
    None,
  );
  let mut config = if let Some(runtime_overrides) = runtime_context_overrides.as_deref() {
    config_loader
      .with_runtime_overrides(runtime_overrides)
      .load()
  } else {
    config_loader.load()
  }
  .map_err(|e| anyhow::anyhow!("load_config: {e}"))?;
  if let Some(overrides) = project_hvigor_overrides.as_ref() {
    overrides
      .apply_to(&mut config, &project_path.join("hvigorfile.ts"))
      .map_err(|error| anyhow::anyhow!("apply project hvigor overrides: {error}"))?;
  }
  if let Some(build_version) = package_args.build_version.as_ref() {
    config.product.build_version = Some(build_version.clone());
  }
  let module_hvigor_overrides = module_hvigor_overrides
    .as_ref()
    .map(|overrides| overrides.bind_to_modules(&config))
    .unwrap_or_default();
  let config = std::sync::Arc::new(config);
  let mut session_builder = BuildSessionBuilder::new(config.clone());
  if let Some(runtime_overrides) = runtime_context_overrides.clone() {
    session_builder = session_builder.with_runtime_overrides(runtime_overrides);
  }
  if let Some(parameter_file_override) = package_args.parameter_file_override.as_ref() {
    session_builder = session_builder
      .with_parameter_file_override(std::path::PathBuf::from(parameter_file_override));
  }
  let session = std::sync::Arc::new(
    session_builder
      .build()
      .map_err(|e| anyhow::anyhow!("session: {e}"))?,
  );

  let args = BuildArgs {
    target,
    mode,
    module,
    product,
    project: Some(project_path),
    build_target_name: package_args.build_target_name,
    build_version: package_args.build_version,
    watch: package_args.watch,
    coverage: package_args.coverage,
    ohos_debug_asan: package_args.ohos_debug_asan,
    ohos_enable_hwasan: package_args.ohos_enable_hwasan,
    preview: package_args.preview,
    disable_abc_cache: package_args.disable_abc_cache || !package_args.cache,
    cache_file: package_args.cache_file.map(std::path::PathBuf::from),
    abc_backend: AbcBackendChoice::Sdk,
    checker: package_args.checker.or_else(|| Some("oxc".to_string())),
    module_analyzer,
    cache: package_args.cache,
    // Hot-rebuild flags are always false when invoked via the Node.js API
    // — only the watch stage sets them for incremental rebuilds.
    skip_resource: false,
    skip_profile: false,
    watch_invalidation: None,
    project_hvigor_overrides,
    module_hvigor_overrides,
    runtime_context_overrides,
    custom_task_input_paths,
    pack_compress_level: package_args.pack_compress_level,
    align_device_types: package_args.align_device_types.unwrap_or_default(),
    collect_debug_symbol: package_args.collect_debug_symbol,
    ui_transform_optimization: package_args.ui_transform_optimization,
    integrated_hsp_use_get_current_bundle_name: package_args
      .integrated_hsp_use_get_current_bundle_name,
    byte_code_har_integrated_optimization: package_args.byte_code_har_integrated_optimization,
    keep_dependency: package_args.keep_dependency,
    parameter_file_override: package_args
      .parameter_file_override
      .map(std::path::PathBuf::from),
    node_executable: package_args
      .runtime_node_executable
      .map(std::path::PathBuf::from),
    native_task_roots,
  };

  let output = if is_app_build {
    let mut coordinator =
      crate::cmd::build::app_coordinator::AppBuildCoordinator::new(session, config, args)
        .with_package_plugins(js_plugins);
    if let Some(task_observer) = task_observer {
      coordinator = coordinator.with_task_observer(task_observer);
    }
    coordinator.execute().await?
  } else {
    let mut coordinator =
      crate::cmd::build::coordinator::BuildCoordinator::new(session, config, args)
        .with_package_plugins(js_plugins);
    if let Some(task_observer) = task_observer {
      coordinator = coordinator.with_task_observer(task_observer);
    }
    coordinator.execute().await?
  };

  Ok(
    output
      .artifact_paths
      .iter()
      .map(|p| arkdown_path::ArtifactPathSerializer::path_to_slash_string(p))
      .collect::<std::result::Result<Vec<_>, _>>()?,
  )
}
