# End-to-End File Flow

本文把 29 个 crate 子文档中的文件、结构体和方法串成一条可执行构建链。它的目标不是替代 `crates/*/files.md`，而是说明这些文件如何按 hvigor 的真实阶段边连接起来。

结论：

- 文件级 API 已经拆到 `crates/*/files.md`；stage-facing 的结构体和方法必须按本文流程串联。
- 本文要求重构后的逻辑与 hvigor 能力完全对齐；不能用 unsupported、静态解析、跳过 hook、跳过 legacy path 替代能力。
- “完全一致”只有在实现阶段通过 parity 测试后才能宣称已验证。当前文档状态是完整 parity contract，不是已实现证明。

## 总流程图

```text
arkdown-cli::main
  -> CliEntrypoint::run
  -> BuildCommand::run
  -> BuildArgsParser::parse
  -> HvigorfileRuntimeBridge::execute_project_script
  -> HvigorfileRuntimeBridge::execute_module_script
  -> HvigorfileRuntimeBridge::normalize_overlay
  -> HvigorfileRuntimeBridge::capture_hooks
  -> HvigorfileRuntimeBridge::capture_task_graph
  -> BuildSessionCommandBuilder::build_session
     -> BuildSessionBuilder::build
        -> HvigorScriptSnapshot::discover
        -> HvigorTaskGraphSnapshot::from_runtime
        -> ProjectSnapshot::from_raw
        -> ModuleSnapshot::from_raw
        -> PackageSnapshot::from_ohpm_result / from_local_module
        -> DependencyGraphBuilder::build
        -> ResolverIndexBuilder::build_for_module
        -> ProfileStore::discover
        -> SdkSnapshot::from_config
        -> ResourceInventory::discover
        -> SourceStore::discover
        -> FingerprintBuilder::build
        -> ArtifactLayout::for_module
  -> BuildPlanBuilder::build
     -> apply HvigorScriptOverlay
     -> splice HvigorTaskGraphSnapshot into BuildRunner
     -> plan_profile / plan_resource / plan_bundle / plan_package / plan_sign
     -> PluginFactory::build_rolldown_plugins
     -> CustomCompilePluginPlan::from_session
  -> BuildRunner::run
     -> DependencyWaveRunner::run_wave
     -> HvigorCustomTaskExecutor::splice_into_runner
     -> ModulePipeline::run
        -> HvigorHookExecutor::run_before_stage
        -> ProfileStage::execute
        -> ResourceStage::execute
        -> BundleStage::execute
        -> Ts2abcStage::execute
        -> PackageStage::execute
        -> SignStage::execute
        -> HvigorHookExecutor::run_after_stage
```

## Hvigor 对齐图

| hvigor 上游阶段 | arkdown 文件/结构体/方法 | 必须保持的等价行为 |
| --- | --- | --- |
| project/module `hvigorfile.ts`、`hvigorconfig.ts` 加载 | `arkdown-cli::cmd/build/hvigorfile_runtime.rs::HvigorfileRuntimeBridge::{execute_project_script,execute_module_script,normalize_overlay,capture_hooks,capture_task_graph}` | 执行 JS/TS 脚本，捕获 overrides、hooks、自定义 task/plugin graph；禁止只静态解析 |
| `projectOhosConfigManager.getOverrides/getHooks`、`BuildOptionPath` | `arkdown-session::hvigor_script.rs::HvigorScriptSnapshot::{apply_overlay_to_project,apply_overlay_to_module,hook_inputs,task_graph}` | build option 优先级与 hvigor 一致，能追踪来源文件和 hook file path |
| `TaskInitializer`、target task graph | `arkdown-session::hvigor_task_graph.rs::HvigorTaskGraphSnapshot::from_runtime` + `arkdown-cli::cmd/build/hvigor_tasks.rs::HvigorCustomTaskExecutor::splice_into_runner` | 自定义 task 和内置 stage 依赖边进入同一个 runner DAG |
| `PluginFactory`、project/module discovery | `arkdown-session::builder.rs::BuildSessionBuilder::{read_project_inputs,read_modules,read_packages,build_inventories}` | 启动阶段一次性读取，后续 stage 不重复探测 |
| `DependencyManager`、`TaskService` views | `arkdown-session::dependency_graph.rs::DependencyGraphBuilder::build` + `dependency_views.rs::*View::from_graph` | HAR/HSP/ByteCode HAR/runtimeOnly/native SO/remote HSP/direct/all/root deps 分类完整 |
| `DefaultSdk` getter family | `arkdown-session::sdk.rs::SdkSnapshot::from_config` + `sdk_tools.rs::SdkToolResolver::*` + `sdk_schemas.rs::SdkSchemaResolver::*` | tool/schema/permission/compile plugin/AOT SDK 输入完整 |
| `GenerateLoaderJson` | `arkdown-profile::loader_json.rs::LoaderJson::from_plan` + `loader_json/bytecode.rs::*` + `loader_json/workers.rs::*` | compile entries、dynamic import、startup、worker/card/form/declaration、ByteCode HAR fields 完整 |
| `ProcessProfile`、router/share/startup/privacy/form/shortcut | `arkdown-profile::{process_profile,router_map,process_share_config,process_startup_config,process_privacy_profile,form_profile}` processors | profile artifacts 写法、校验和 dependency merge 与 hvigor 一致 |
| `ProcessResource`、`CompileResource`、`ProcessCompiledResources` | `arkdown-resource::ResourceCompiler::compile` + `code_resource.rs::CodeResourceCopier::copy` + `compiled.rs::CompiledResourceProcessor::process_compiled_resources` | restool 资源、源码非代码资源 copy、preload system so json、preview incremental 完整 |
| `ArkCompile.initDefaultArkCompileConfig` | `arkdown-cli::compile_env.rs::*Planner` + `arkdown::compile_env.rs::CompileEnvironment::from_plan` | compileEnv、buildMode、compileMode、watch、cache、manualChunks、source map policy 完整 |
| `arkts-pack.js::getBuiltInPlugins/getPlugins` | `arkdown-cli::plugins.rs::PluginFactory::build_rolldown_plugins` + `plugin_order.rs::PluginOrderPlanner::*` + `custom_plugins.rs::CustomPluginRuntimeBridge::execute_hook` | 内置 plugin 顺序、自定义 plugin order `first/last/function`、hook 执行完整 |
| `arkts-pack.js::doWrapper` lifecycle | `arkdown::plugin_lifecycle.rs::RolldownLifecycleBridge::{run_build_start,run_resolve_id,run_load,run_transform,run_module_parsed,run_before_build_end,run_build_end,run_after_build_end,run_clean_up}` | Rollup hook 语义通过 Rolldown hook、pre/post bridge 或 toolchain delegate 完整保留 |
| SDK `visualTransform -> etsTransform -> apiTransform` | `arkdown-ets-transform::pipeline.rs::EtsTransformer::{run_visual,run_checker,run_arkui,run_api,emit}` | `.ets/.ts/.js/.visual/.d.ets` 分流、ArkUI lowering、API/libso/kit facts 完整 |
| TypeScript Program、Arkoala、const enum invalidation | `arkdown-ets-transform::semantic.rs::SemanticProgramBridge::prepare_program` + `arkoala.rs::ArkoalaTransformGate::*` + `transform_cache.rs::*` | OXC 只替代 AST/codegen；semantic/checker/cache 分支必须保留 |
| `rollup-plugin-gen-abc.js` | `arkdown-ts2abc::compiler.rs::Ts2abcCompiler::compile` + `module_mode.rs::ModuleAbcGenerator::generate_module_abc` + `bundle_mode.rs::BundleAbcGenerator::generate_bundle_abc` | module/bundle/single-file ABC 与 SDK genAbc 对齐 |
| `gen_aot.js`、legacy `gen_abc_plugin.js`、`run-ark.js` | `arkdown-ts2abc::aot.rs::AotCompiler::compile_aot` + `legacy_bundle.rs::LegacyAbcCompiler::{compile,delegate_to_legacy_toolchain}` | AOT 和 legacy/fallback path 必须实现或委托上游等价 toolchain |
| `ark_compiler/transform.js` JS compat/lazy import | `arkdown-ts2abc::module_source.rs::{process_lazy_import,apply_js_api10_compat}` + `js_compat.rs::JsCompatTransform::transform_api10_js` | API <= 10 JS Babel branch、lazy import/reexport checks 完整 |
| bytecode obfuscator / terser | `arkdown-obfuscate::Obfuscator::obfuscate` + `arkdown-ts2abc::bytecode_obfuscation.rs::BytecodeObfuscator::run` | name cache、bytecode obfuscation、HAR ability rewrite 输入完整 |
| `PackageHap`、`PackageHsp`、`PackageHar` | `arkdown-pack::Packager::{package_hap,package_hsp,package_har}` | HAP/HSP/HAR package entry order、pack info、native/debug symbol 完整 |
| `ProcessHarArtifacts` | `arkdown-pack::{har_artifacts,har_router_startup,har_obfuscation,har_insight_intent,native_preload}` processors | compiled source staging、bundle dep res、router/startup/share、obfuscated ability、insight intent、native preload 完整 |
| `SignHap`、`SignHsp`、`PackageSignHar` | `arkdown-sign::Signer::sign` + sign model builders | 签名输入、签名模型、profile/cert/key path 由 plan 提供，不 late-read |
| `invalid-cache-plugin.js`、rollup cache、watch change | `arkdown-cache::{plugin_cache,rollup_cache,event_name}` + `arkdown-watcher::event_name.rs::WatchEventNameMapper::event_name_for` | cache key、shouldInvalidCache、watch event name 与 hvigor 对齐 |
| preview/unit/ohosTest/reload/coverage | `arkdown-cli::{preview,test,reload,coverage}` stage executors | mode-specific task graph 进入同一 session/runner，不另起扫描链 |

## 单模块 Pipeline 细化图

```text
ModulePipeline::run
  -> ProfileStage::execute
     -> ProfileProcessor::new(plan).process()
     -> ModuleJsonWriter::write
     -> LoaderJson::from_plan -> LoaderJsonWriter::write
     -> PkgContextInfo::from_plan -> PkgContextInfo::write
     -> OhPackageWriter::write_har_oh_package
  -> ResourceStage::execute
     -> ResourceCompiler::new(plan).compile()
     -> RestoolCommandBuilder::build_command
     -> CodeResourceCopier::copy
     -> CompiledResourceProcessor::process_compiled_resources
  -> BundleStage::execute
     -> PluginFactory::build_rolldown_plugins
     -> CustomCompilePluginPlan::load_sources -> CustomPluginRuntimeBridge::execute_hook
     -> Bundler::new(plan).bundle()
        -> RolldownLifecycleBridge::run_build_start
        -> OhosResolvePlugin::resolve_id
        -> Json5Plugin::load / transform
        -> EtsTransformPlugin::transform
           -> EtsTransformer::transform
              -> OxcSourceParser::parse_for_kind
              -> SemanticProgramBridge::prepare_program
              -> VisualTransform::transform
              -> EtsChecker::check_file
              -> UiSyntaxPass::lower
              -> ApiTransform::rewrite_api_imports
        -> RolldownLifecycleBridge::run_before_build_end
        -> RolldownLifecycleBridge::run_build_end
        -> BundleFacts::merge_plugin_facts
  -> Ts2abcStage::execute
     -> Ts2abcPlan::from_loader_and_bundle
     -> Ts2abcCompiler::compile
        -> ModuleAbcGenerator::generate_module_abc
        -> BundleAbcGenerator::generate_bundle_abc
        -> AotCompiler::compile_aot
        -> LegacyAbcCompiler::delegate_to_legacy_toolchain
        -> AbcCompiler::new(plan).compile()
  -> PackageStage::execute
     -> Packager::new(plan).package()
     -> HarArtifactProcessor::process
     -> NativeLibPackPlan::entries_from_inventory
  -> SignStage::execute
     -> Signer::new(plan).sign()
```

## 串联检查规则

每个 `files.md` 的行必须满足三条检查：

1. `方法/关联函数` 列只能是结构体、枚举或 trait 方法；不能规划 public free function。
2. stage-facing 方法必须能在本文的总流程图或单模块 pipeline 中找到上游/下游调用点。
3. leaf helper 文件如果不直接出现在流程图，必须由同 crate 的 stage-facing type 调用，并在 `对应上游逻辑` 列写到具体 hvigor/SDK 文件、class 或函数。

## Parity 验证入口

实现阶段必须以本文流程图为测试分组：

- `hvigorfile-runtime`: project/module script、overrides、hooks、custom task graph、`arkOptions.compilePluginFile`。
- `profile-loader`: `GenerateLoaderJson` dynamic/startup/worker/bytecode/declaration branches。
- `resource`: restool、copy code resource、compiled resources、preview incremental。
- `bundle-lifecycle`: built-in plugin order、自定义 plugin order/hook、Rollup lifecycle bridge。
- `ets-transform`: `.ets/.ts/.js/.visual/.d.ets`、TypeScript semantic、Arkoala、API/libso/kit facts。
- `abc`: module/bundle/single-file、AOT、legacy fallback、API <= 10 JS compat。
- `pack-sign`: HAP/HSP/HAR、ProcessHarArtifacts、shared HAR/TGZ、native preload、sign。
- `watch-cache`: shouldInvalidCache、rollup cache、watch event names、hot/cold reload。

只有这些分组与同一 fixture 的 hvigor 输出、诊断和可观察 side effects 对齐后，才能说实现已经完全一致。

