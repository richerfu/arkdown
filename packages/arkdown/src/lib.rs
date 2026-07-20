//! arkdown Node.js bindings (napi-rs 3.9.x).
//!
//! Exposes a `build()` function that accepts Rolldown-compatible JS plugin
//! objects. Plugin hooks are called via NAPI `ThreadsafeFunction` — same
//! process, same V8 heap, no subprocess overhead.

#![deny(clippy::all)]

#[cfg(all(
  feature = "allocator",
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

pub mod cmd;
pub mod progress;
pub mod task_cache;

use std::borrow::Cow;
use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction};
use napi_derive::napi;
use tracing_subscriber::EnvFilter;

use rolldown_common::{ModuleInfo, NormalModule};
use rolldown_plugin::{
  HookBuildEndArgs, HookBuildStartArgs, HookLoadArgs, HookLoadOutput, HookLoadReturn,
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookTransformArgs,
  HookTransformOutput, HookTransformReturn, HookUsage, Plugin, PluginContext,
  SharedLoadPluginContext, SharedTransformPluginContext,
};

use arkdown::plugins::external::PluginOrder;
use arkdown::plugins::factory::{PackagePlugin, PackagePluginRuntimeHooks};
use arkdown::ShouldInvalidCacheRequest;

// ---------------------------------------------------------------------------
// Type aliases for the ThreadsafeFunction types
// ---------------------------------------------------------------------------

// CalleeHandled=false: build_callback returns this variant; call_async(value) not call_async(Ok(value))
type ResolveIdTsfn = Arc<
  ThreadsafeFunction<
    (String, Option<String>),
    Option<String>,
    FnArgs<(String, Option<String>)>,
    napi::Status,
    false,
  >,
>;
type LoadTsfn =
  Arc<ThreadsafeFunction<String, Option<String>, FnArgs<(String,)>, napi::Status, false>>;
type TransformTsfn = Arc<
  ThreadsafeFunction<
    (String, String),
    Option<String>,
    FnArgs<(String, String)>,
    napi::Status,
    false,
  >,
>;
type NoopTsfn = Arc<ThreadsafeFunction<String, (), FnArgs<(String,)>, napi::Status, false>>;
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
  // ── Rolldown bundler hooks ───────────────────────────────────────────────
  resolve_id_fn: Option<ResolveIdTsfn>,
  load_fn: Option<LoadTsfn>,
  transform_fn: Option<TransformTsfn>,
  build_start_fn: Option<NoopTsfn>,
  module_parsed_fn: Option<NoopTsfn>,
  before_build_end_fn: Option<NoopTsfn>,
  build_end_fn: Option<NoopTsfn>,
  /// `resolveDynamicImport` — same TSFN shape as `resolveId`.
  ///
  /// Corresponds to hvigor's `resolveDynamicImport` hook listed in
  /// `doWrapper` event names.
  resolve_dynamic_import_fn: Option<ResolveIdTsfn>,
  // ── Cache/transform decision hooks ──────────────────────────────────────
  should_invalid_cache_fn: Option<BoolTsfn>,
  should_transform_cached_module_fn: Option<BoolTsfn>,
  // ── Coordinator lifecycle hooks ──────────────────────────────────────────
  /// `beforeBuild` — hvigor calls this before `runArkPack()` starts.
  ///
  /// Called with a JSON share context: `{ share: { bundleName, buildMode, … } }`.
  before_build_fn: Option<NoopTsfn>,
  /// `afterBuildStart` — hvigor calls this after Rolldown `buildStart` finishes.
  after_build_start_fn: Option<NoopTsfn>,
  /// `afterBuildEnd` — hvigor calls this after `buildEnd` and `cleanUp`.
  ///
  /// Called with `{ success: bool, share: { … } }`.
  after_build_end_fn: Option<NoopTsfn>,
  /// `cleanUp` — resource cleanup; always called, even on error.
  clean_up_fn: Option<NoopTsfn>,
  // ── Diagnostic context handlers ──────────────────────────────────────────
  /// `warn` — forward warning diagnostics from Rust pipeline to plugin.
  warn_fn: Option<NoopTsfn>,
  /// `error` — forward error diagnostics from Rust pipeline to plugin.
  error_fn: Option<NoopTsfn>,
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
  fn from_js_object(obj: Object<'_>) -> napi::Result<(Arc<Self>, PluginOrder)> {
    let name: String = obj
      .get::<String>("name")?
      .unwrap_or_else(|| "unknown".to_string());
    let order = match obj.get::<String>("order")?.as_deref() {
      Some("first") => PluginOrder::First,
      Some("last") | None => PluginOrder::Last,
      Some(other) => {
        tracing::warn!(
          plugin = %name,
          order = %other,
          "unsupported JS plugin order, using 'last'"
        );
        PluginOrder::Last
      }
    };

    let mut usage = HookUsage::empty();

    let build_start_fn = if let Some(js_fn) = obj.get::<Function<(String,), ()>>("buildStart")? {
      let tsfn = js_fn
        .build_threadsafe_function::<String>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
      usage |= HookUsage::BuildStart;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let resolve_id_fn = if let Some(js_fn) =
      obj.get::<Function<(String, Option<String>), Option<String>>>("resolveId")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(String, Option<String>)>()
        .build_callback(|ctx: ThreadsafeCallContext<(String, Option<String>)>| {
          Ok(FnArgs { data: ctx.value })
        })?;
      usage |= HookUsage::ResolveId;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let load_fn = if let Some(js_fn) = obj.get::<Function<(String,), Option<String>>>("load")? {
      let tsfn = js_fn
        .build_threadsafe_function::<String>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
      usage |= HookUsage::Load;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let transform_fn =
      if let Some(js_fn) = obj.get::<Function<(String, String), Option<String>>>("transform")? {
        let tsfn = js_fn
          .build_threadsafe_function::<(String, String)>()
          .build_callback(|ctx: ThreadsafeCallContext<(String, String)>| {
            Ok(FnArgs { data: ctx.value })
          })?;
        usage |= HookUsage::Transform;
        Some(Arc::new(tsfn))
      } else {
        None
      };

    let module_parsed_fn =
      if let Some(js_fn) = obj.get::<Function<(String,), ()>>("moduleParsed")? {
        let tsfn = js_fn
          .build_threadsafe_function::<String>()
          .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
        usage |= HookUsage::ModuleParsed;
        Some(Arc::new(tsfn))
      } else {
        None
      };

    let before_build_end_fn =
      if let Some(js_fn) = obj.get::<Function<(String,), ()>>("beforeBuildEnd")? {
        let tsfn = js_fn
          .build_threadsafe_function::<String>()
          .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
        Some(Arc::new(tsfn))
      } else {
        None
      };

    let build_end_fn = if let Some(js_fn) = obj.get::<Function<(String,), ()>>("buildEnd")? {
      let tsfn = js_fn
        .build_threadsafe_function::<String>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
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

    let should_transform_cached_module_fn =
      if let Some(js_fn) = obj.get::<Function<(String,), bool>>("shouldTransformCachedModule")? {
        let tsfn = js_fn
          .build_threadsafe_function::<String>()
          .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
        Some(Arc::new(tsfn))
      } else {
        None
      };

    // ── resolveDynamicImport — same TSFN shape as resolveId ─────────────────
    let resolve_dynamic_import_fn = if let Some(js_fn) =
      obj.get::<Function<(String, Option<String>), Option<String>>>("resolveDynamicImport")?
    {
      let tsfn = js_fn
        .build_threadsafe_function::<(String, Option<String>)>()
        .build_callback(|ctx: ThreadsafeCallContext<(String, Option<String>)>| {
          Ok(FnArgs { data: ctx.value })
        })?;
      usage |= HookUsage::ResolveDynamicImport;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    // ── Coordinator lifecycle hooks ───────────────────────────────────────
    // beforeBuild / afterBuildStart / afterBuildEnd / cleanUp.
    // Each receives a JSON share context string so plugins can read build
    // metadata as `this.share` via runtime.js normalizeHookArguments.
    let before_build_fn = if let Some(js_fn) = obj.get::<Function<(String,), ()>>("beforeBuild")? {
      let tsfn = js_fn
        .build_threadsafe_function::<String>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let after_build_start_fn =
      if let Some(js_fn) = obj.get::<Function<(String,), ()>>("afterBuildStart")? {
        let tsfn = js_fn
          .build_threadsafe_function::<String>()
          .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
        Some(Arc::new(tsfn))
      } else {
        None
      };

    let after_build_end_fn =
      if let Some(js_fn) = obj.get::<Function<(String,), ()>>("afterBuildEnd")? {
        let tsfn = js_fn
          .build_threadsafe_function::<String>()
          .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
        Some(Arc::new(tsfn))
      } else {
        None
      };

    let clean_up_fn = if let Some(js_fn) = obj.get::<Function<(String,), ()>>("cleanUp")? {
      let tsfn = js_fn
        .build_threadsafe_function::<String>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    // Plugin context hooks: warn/error are diagnostic handlers called by the
    // Rust build pipeline to notify the plugin of build warnings/errors.
    // Corresponds to hvigor's `context.warn` / `context.error` in `getCompilePluginContext`.
    let warn_fn = if let Some(js_fn) = obj.get::<Function<(String,), ()>>("warn")? {
      let tsfn = js_fn
        .build_threadsafe_function::<String>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    let error_fn = if let Some(js_fn) = obj.get::<Function<(String,), ()>>("error")? {
      let tsfn = js_fn
        .build_threadsafe_function::<String>()
        .build_callback(|ctx: ThreadsafeCallContext<String>| Ok(FnArgs { data: (ctx.value,) }))?;
      Some(Arc::new(tsfn))
    } else {
      None
    };

    tracing::debug!(plugin = %name, "wrapped JS plugin");
    Ok((
      Arc::new(Self {
        name,
        resolve_id_fn,
        load_fn,
        transform_fn,
        build_start_fn,
        module_parsed_fn,
        before_build_end_fn,
        build_end_fn,
        resolve_dynamic_import_fn,
        should_invalid_cache_fn,
        should_transform_cached_module_fn,
        before_build_fn,
        after_build_start_fn,
        after_build_end_fn,
        clean_up_fn,
        warn_fn,
        error_fn,
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
    _ctx: &PluginContext,
    _args: &HookBuildStartArgs<'_>,
  ) -> impl std::future::Future<Output = rolldown_plugin::HookNoopReturn> + Send {
    let tsfn = self.build_start_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(()) };
      f.call_async(String::new())
        .await
        .map_err(|e| anyhow::anyhow!("buildStart JS error: {e}"))?;
      Ok(())
    }
  }

  fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
    let specifier = args.specifier.to_string();
    let importer = args.importer.map(|s| s.to_string());
    let tsfn = self.resolve_id_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(None) };
      match f.call_async((specifier, importer)).await {
        Ok(Some(id)) => Ok(Some(HookResolveIdOutput {
          id: id.into(),
          ..Default::default()
        })),
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
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
    let specifier = args.specifier.to_string();
    let importer = args.importer.map(|s| s.to_string());
    let tsfn = self.resolve_dynamic_import_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(None) };
      match f.call_async((specifier, importer)).await {
        Ok(Some(id)) => Ok(Some(HookResolveIdOutput {
          id: id.into(),
          ..Default::default()
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("resolveDynamicImport JS error: {e}")),
      }
    }
  }

  fn load(
    &self,
    _ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    let id = args.id.to_string();
    let tsfn = self.load_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(None) };
      match f.call_async(id).await {
        Ok(Some(code)) => Ok(Some(HookLoadOutput {
          code: code.into(),
          ..Default::default()
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("load JS error: {e}")),
      }
    }
  }

  fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
    let code = args.code.to_string();
    let id = args.id.to_string();
    let tsfn = self.transform_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(None) };
      match f.call_async((code, id)).await {
        Ok(Some(new_code)) => Ok(Some(HookTransformOutput {
          code: Some(new_code),
          ..Default::default()
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("transform JS error: {e}")),
      }
    }
  }

  fn module_parsed(
    &self,
    _ctx: &PluginContext,
    module_info: Arc<ModuleInfo>,
    _normal_module: &NormalModule,
  ) -> impl std::future::Future<Output = rolldown_plugin::HookNoopReturn> + Send {
    let id = module_info.id.to_string();
    let tsfn = self.module_parsed_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(()) };
      f.call_async(id)
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
    let error_count = args
      .map(|args| args.errors.len().to_string())
      .unwrap_or_default();

    // Collect module info from the Rolldown module graph synchronously
    // (PluginContext is not Send, so this must happen outside the async block).
    //
    // Builds a map: moduleId → { importedIds, dynamicallyImportedIds }
    // matching the shape hvigor provides through `context.getModuleInfo(id)`.
    //
    // # Upstream correspondence
    //
    // Corresponds to hvigor's `invalid-cache-plugin.ts::buildEnd`:
    //   for (const t of this.getModuleIds()) {
    //     const a = this.getModuleInfo(t);
    //     e.set(t, a ? a.importedIds.concat(a.dynamicallyImportedIds) : []);
    //   }
    // The serialized map is passed to JS as `{ moduleInfoMap: { id: { importedIds, dynamicallyImportedIds } } }`.
    let module_info_json: String = {
      let mut map = serde_json::Map::new();
      for module_id in ctx.get_module_ids() {
        let id_str = module_id.as_str();
        let (imported, dyn_imported): (Vec<String>, Vec<String>) =
          if let Some(info) = ctx.get_module_info(id_str) {
            let imported = info.imported_ids.iter().map(|id| id.to_string()).collect();
            let dyn_imported = info
              .dynamically_imported_ids
              .iter()
              .map(|id| id.to_string())
              .collect();
            (imported, dyn_imported)
          } else {
            (vec![], vec![])
          };
        map.insert(
          id_str.to_string(),
          serde_json::json!({
            "importedIds": imported,
            "dynamicallyImportedIds": dyn_imported,
          }),
        );
      }
      // Serialise the context object that hvigor's buildEnd plugin receives.
      // JS side: buildEnd(ctx) where ctx.errorCount and ctx.moduleInfoMap are available.
      serde_json::to_string(&serde_json::json!({
        "errorCount": &error_count,
        "moduleInfoMap": serde_json::Value::Object(map),
      }))
      .unwrap_or_else(|_| error_count.clone())
    };

    let tsfn = self.build_end_fn.clone();
    async move {
      let Some(f) = tsfn else { return Ok(()) };
      f.call_async(module_info_json)
        .await
        .map_err(|e| anyhow::anyhow!("buildEnd JS error: {e}"))?;
      Ok(())
    }
  }
}

#[async_trait::async_trait]
impl PackagePluginRuntimeHooks for JsPlugin {
  // ── Coordinator lifecycle hooks ────────────────────────────────────────

  async fn before_build_with_share(&self, share_json: &str) -> anyhow::Result<()> {
    let Some(f) = self.before_build_fn.clone() else {
      return Ok(());
    };
    f.call_async(share_json.to_string())
      .await
      .map_err(|e| anyhow::anyhow!("beforeBuild JS error: {e}"))?;
    Ok(())
  }

  async fn after_build_start_with_share(&self, share_json: &str) -> anyhow::Result<()> {
    let Some(f) = self.after_build_start_fn.clone() else {
      return Ok(());
    };
    f.call_async(share_json.to_string())
      .await
      .map_err(|e| anyhow::anyhow!("afterBuildStart JS error: {e}"))?;
    Ok(())
  }

  /// `afterBuildEnd` — called after `buildEnd` with success status.
  async fn after_build_end(&self, success: bool) -> anyhow::Result<()> {
    let Some(f) = self.after_build_end_fn.clone() else {
      return Ok(());
    };
    let ctx = serde_json::json!({ "success": success }).to_string();
    f.call_async(ctx)
      .await
      .map_err(|e| anyhow::anyhow!("afterBuildEnd JS error: {e}"))?;
    Ok(())
  }

  /// `cleanUp` — resource cleanup; always called, even on error.
  async fn clean_up(&self) -> anyhow::Result<()> {
    let Some(f) = self.clean_up_fn.clone() else {
      return Ok(());
    };
    f.call_async(String::new())
      .await
      .map_err(|e| anyhow::anyhow!("cleanUp JS error: {e}"))?;
    Ok(())
  }

  async fn should_invalid_cache(
    &self,
    request: &ShouldInvalidCacheRequest,
  ) -> anyhow::Result<bool> {
    let Some(f) = self.should_invalid_cache_fn.clone() else {
      return Ok(false);
    };
    let request_json = serde_json::json!({
      "id": request.module_name.as_deref().unwrap_or_default(),
      "meta": {
        "moduleName": &request.module_name,
        "cacheDir": request
          .cache_dir
          .as_ref()
          .map(|path| arkdown_path::ArtifactPathSerializer::path_to_slash_string(path))
          .transpose()?,
        "compileEnv": &request.compile_env,
      },
      "changedFiles": request
        .changed_files
        .iter()
        .map(|path| arkdown_path::ArtifactPathSerializer::path_to_slash_string(path))
        .collect::<std::result::Result<Vec<_>, _>>()?,
      "isWatch": request.is_watch,
      "cacheHit": request.cache_hit,
    })
    .to_string();
    f.call_async(request_json)
      .await
      .map_err(|e| anyhow::anyhow!("shouldInvalidCache JS error: {e}"))
  }

  async fn before_build_end(&self) -> anyhow::Result<()> {
    self.before_build_end_with_context(&[]).await
  }

  async fn before_build_end_with_context(&self, module_ids: &[String]) -> anyhow::Result<()> {
    let Some(f) = self.before_build_end_fn.clone() else {
      return Ok(());
    };
    // Serialise module IDs as a JSON context object so plugins can call
    // `getModuleIds()` inside `beforeBuildEnd`.  Matches hvigor's
    // `getCompilePluginContext(this).getModuleIds()` surface.
    //
    // The hook signature on the JS side is:
    //   `beforeBuildEnd(ctx?: { moduleIds: string[] }): void`
    //
    // For backwards compatibility with plugins that take no argument, the
    // empty-string token is used when no module IDs are available.
    let ctx_json = if module_ids.is_empty() {
      String::new()
    } else {
      serde_json::json!({ "moduleIds": module_ids }).to_string()
    };
    f.call_async(ctx_json)
      .await
      .map_err(|e| anyhow::anyhow!("beforeBuildEnd JS error: {e}"))?;
    Ok(())
  }

  async fn should_transform_cached_module(
    &self,
    request: &ShouldInvalidCacheRequest,
  ) -> anyhow::Result<bool> {
    let Some(f) = self.should_transform_cached_module_fn.clone() else {
      return Ok(false);
    };
    let request_json = serde_json::json!({
      "id": request.module_name.as_deref().unwrap_or_default(),
      "meta": {
        "moduleName": &request.module_name,
        "cacheDir": request
          .cache_dir
          .as_ref()
          .map(|path| arkdown_path::ArtifactPathSerializer::path_to_slash_string(path))
          .transpose()?,
        "compileEnv": &request.compile_env,
      },
      "changedFiles": request
        .changed_files
        .iter()
        .map(|path| arkdown_path::ArtifactPathSerializer::path_to_slash_string(path))
        .collect::<std::result::Result<Vec<_>, _>>()?,
      "isWatch": request.is_watch,
      "cacheHit": request.cache_hit,
    })
    .to_string();
    f.call_async(request_json)
      .await
      .map_err(|e| anyhow::anyhow!("shouldTransformCachedModule JS error: {e}"))
  }

  async fn warn(&self, message: &str) -> anyhow::Result<()> {
    let Some(f) = self.warn_fn.clone() else {
      return Ok(());
    };
    f.call_async(message.to_string())
      .await
      .map_err(|e| anyhow::anyhow!("warn JS error: {e}"))?;
    Ok(())
  }

  async fn error(&self, message: &str) -> anyhow::Result<()> {
    let Some(f) = self.error_fn.clone() else {
      return Ok(());
    };
    f.call_async(message.to_string())
      .await
      .map_err(|e| anyhow::anyhow!("error JS error: {e}"))?;
    Ok(())
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
    // Cache/transform hooks
    if self.should_invalid_cache_fn.is_some() {
      hooks.push("shouldInvalidCache");
    }
    if self.should_transform_cached_module_fn.is_some() {
      hooks.push("shouldTransformCachedModule");
    }
    // Coordinator lifecycle hooks
    if self.before_build_fn.is_some() {
      hooks.push("beforeBuild");
    }
    if self.after_build_start_fn.is_some() {
      hooks.push("afterBuildStart");
    }
    if self.after_build_end_fn.is_some() {
      hooks.push("afterBuildEnd");
    }
    if self.clean_up_fn.is_some() {
      hooks.push("cleanUp");
    }
    // Diagnostic handlers
    if self.warn_fn.is_some() {
      hooks.push("warn");
    }
    if self.error_fn.is_some() {
      hooks.push("error");
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
  pub imported_ids: Vec<String>,
  pub dynamically_imported_ids: Vec<String>,
}

/// Compile plugin context — the `this` value inside user plugin hooks.
///
/// Runtime JS binds this object before forwarding hooks into the Rust NAPI
/// bridge. Methods are typed here for generated TypeScript declarations; the
/// actual method implementations live in `runtime.js`.
#[napi(object)]
pub struct HvigorPluginContext {
  pub share: Option<HvigorShareContext>,
  #[napi(ts_type = "() => string[]")]
  pub get_module_ids: Option<String>,
  #[napi(ts_type = "(id: string) => HvigorModuleInfo | null")]
  pub get_module_info: Option<String>,
  #[napi(ts_type = "(source: string, options?: unknown) => unknown")]
  pub parse: Option<String>,
  #[napi(ts_type = "(moduleRef: string) => string | null")]
  pub get_file_name: Option<String>,
}

/// Cache invalidation request passed to `shouldInvalidCache` and
/// `shouldTransformCachedModule`.
#[napi(object)]
pub struct PluginCacheRequest {
  pub id: String,
  pub meta: Option<PluginCacheRequestMeta>,
  pub changed_files: Vec<String>,
  pub is_watch: bool,
  pub cache_hit: bool,
}

/// Cache request metadata.
#[napi(object)]
pub struct PluginCacheRequestMeta {
  pub module_name: Option<String>,
  pub cache_dir: Option<String>,
  pub compile_env: Option<String>,
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
  #[napi(ts_type = "'first' | 'last' | ((nameToIndex: ReadonlyMap<string, number>) => number)")]
  pub order: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, ctxJson?: string) => void | Promise<void>")]
  pub before_build: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, ctxJson?: string) => void | Promise<void>")]
  pub build_start: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, ctxJson?: string) => void | Promise<void>")]
  pub after_build_start: Option<String>,
  #[napi(
    ts_type = "(id: string, importer?: string | null) => string | null | undefined | Promise<string | null | undefined>"
  )]
  pub resolve_id: Option<String>,
  #[napi(
    ts_type = "(id: string) => string | null | undefined | Promise<string | null | undefined>"
  )]
  pub load: Option<String>,
  #[napi(
    ts_type = "(code: string, id: string) => string | null | undefined | Promise<string | null | undefined>"
  )]
  pub transform: Option<String>,
  #[napi(ts_type = "(id: string) => void | Promise<void>")]
  pub module_parsed: Option<String>,
  #[napi(
    ts_type = "(id: string, importer?: string | null) => string | null | undefined | Promise<string | null | undefined>"
  )]
  pub resolve_dynamic_import: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, ctxJson?: string) => void | Promise<void>")]
  pub before_build_end: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, ctxJson?: string) => void | Promise<void>")]
  pub build_end: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, ctxJson?: string) => void | Promise<void>")]
  pub after_build_end: Option<String>,
  #[napi(ts_type = "(this: HvigorPluginContext, ctxJson?: string) => void | Promise<void>")]
  pub clean_up: Option<String>,
  #[napi(ts_type = "(request: PluginCacheRequest) => boolean | Promise<boolean>")]
  pub should_invalid_cache: Option<String>,
  #[napi(ts_type = "(request: PluginCacheRequest) => boolean | Promise<boolean>")]
  pub should_transform_cached_module: Option<String>,
  #[napi(ts_type = "(message: string) => void | Promise<void>")]
  pub warn: Option<String>,
  #[napi(ts_type = "(message: string) => void | Promise<void>")]
  pub error: Option<String>,
}

#[napi(object)]
pub struct BuildConfig {
  pub project: Option<String>,
  pub target: Option<String>,
  pub mode: Option<String>,
  pub module: Option<String>,
  pub product: Option<String>,
  pub build_target_name: Option<String>,
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
#[napi(ts_return_type = "Promise<BuildResult>")]
pub fn build(
  env: Env,
  config: BuildConfig,
  #[napi(ts_arg_type = "Array<ArkdownPlugin> | undefined | null")] plugins: Option<Array<'_>>,
) -> napi::Result<Object<'_>> {
  init_napi_tracing_from_env();

  // 1. Wrap JS plugins synchronously (Env + Array are not Send).
  let mut js_plugins: Vec<PackagePlugin> = Vec::new();
  if let Some(arr) = plugins {
    let len = arr.len();
    for i in 0..len {
      if let Some(obj) = arr.get::<Object<'_>>(i)? {
        match JsPlugin::from_js_object(obj) {
          Ok((p, order)) => {
            let plugin = p.clone() as rolldown_plugin::__inner::SharedPluginable;
            let runtime_hooks = p as Arc<dyn PackagePluginRuntimeHooks>;
            js_plugins.push(PackagePlugin::with_runtime_hooks(
              plugin,
              order,
              runtime_hooks,
            ));
          }
          Err(e) => tracing::warn!(index = i, error = %e, "failed to wrap plugin[{i}]"),
        }
      }
    }
  }

  // Extract all config values (Send types only).
  let BuildConfig {
    project,
    target,
    mode,
    module,
    product,
    build_target_name,
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
  } = config;
  let project = project.unwrap_or_else(|| ".".to_string());
  let target = target.unwrap_or_else(|| "hap".to_string());
  let mode = mode.unwrap_or_else(|| "debug".to_string());
  let package_args = PackageBuildArgs {
    build_target_name,
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
  use crate::cmd::build::args::{AbcBackendChoice, ModuleAnalyzerChoice};
  use crate::cmd::build::BuildArgs;
  use arkdown_config::load_config;
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
  let output_target = match target.as_str() {
    "hsp" => OutputTarget::Hsp,
    "har" => OutputTarget::Har,
    _ => OutputTarget::Hap,
  };
  let build_mode = match mode.as_str() {
    "release" => BuildMode::Release,
    _ => BuildMode::Debug,
  };

  let config = std::sync::Arc::new(
    load_config(
      &project_path,
      build_mode,
      output_target,
      product.as_deref(),
      None,
    )
    .map_err(|e| anyhow::anyhow!("load_config: {e}"))?,
  );
  let session = std::sync::Arc::new(
    BuildSessionBuilder::new(config.clone())
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
  };

  let output = crate::cmd::build::coordinator::BuildCoordinator::new(session, config, args)
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
