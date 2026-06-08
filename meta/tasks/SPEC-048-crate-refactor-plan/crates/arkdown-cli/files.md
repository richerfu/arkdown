# File API Plan: crates/arkdown-cli

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- CLI 是唯一组合所有 stage/plugin crate 的层。
- 当前 `src/cmd/build/*` 已经有很多阶段文件，但仍承担 session、artifact layout、package entry、ABC handoff 等 domain policy；需要拆成 `args/session/plan/plugins/runner/pipeline/stages/artifacts/cache`，并删除 `compile_arkts_handoff.rs` 这类临时桥接文件。
- 所有 handoff/bridge/intermediate 类文件都只能作为迁移来源。数据迁入 typed plan、stage output、artifact writer、cache owner 或 package plan 后，原文件必须删除，不能以 debug/parity/compat 名义保留。
- 对应 hvigor `TaskInitializer` 和 `ArkCompile` worker/job 调度，但不复制 task class hierarchy。
- CLI 是唯一可以读取 `hvigorPluginFile`、`compilePluginPath` 并组合自定义编译插件兼容策略的层；domain crate 和 `arkdown` 都不能直接读这些 JS 配置。
- `hvigorfile.ts`/`hvigorconfig.ts` 是更上游的项目/模块脚本入口。CLI 必须通过 hvigor-compatible JS/TS runtime bridge 执行这些脚本、hooks 和自定义 tasks，并把 normalized overlay 与 task graph 交给 `BuildSessionBuildInput`；不能把它误认为 ArkTS compile plugin。
- BuildPlanBuilder 必须显式保留上游 `arkts-pack.js::getPlugins/getBuiltInPlugins/generateConfig` 的顺序和可观察字段；自定义插件路径必须通过 `CustomPluginRuntimeBridge` 执行，不能降级成 hard diagnostic。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `main.rs` | `Cli`, `Commands`, `CliEntrypoint`, `TracingInitializer` | `CliEntrypoint::run()`, `TracingInitializer::init()`; binary `fn main` only delegates and is not exported | current `main.rs` | command entry |
| `lib.rs` | `CliResult` | exports cmd modules | current `lib.rs` | CLI crate boundary |
| `progress.rs` | `Progress`, `StageProgress`, `ModuleProgress` | `start`, `finish`, `stage`, `message` | current `Progress` | hvigor task events |
| `diagnostics.rs` | `CliDiagnosticRenderer` | `render`, `render_error`, `render_warning` | new | hvigor logger output |
| `cmd/mod.rs` | `CommandRunner` | `CommandRunner::run_command()` | new | command dispatch |
| `cmd/build/mod.rs` | `BuildCommand` | `BuildCommand::run()` | current build entry | build command |
| `cmd/build/args.rs` | `BuildArgs`, `ArtifactRequest`, `BuildSelection`, `BuildArgsParser` | `BuildArgsParser::parse()`, `BuildArgs::artifact_kind()`, `BuildArgs::target_name()`, `BuildArgs::build_mode()` | current build CLI parsing | assemble task selection |
| `cmd/build/session.rs` | `BuildSessionCommandInput`, `BuildSessionCommandBuilder` | `BuildSessionCommandBuilder::from_args()`, `BuildSessionCommandBuilder::build_session()` | current config/project discovery | `PluginFactory`, `PreBuild` |
| `cmd/build/hvigorfile_runtime.rs` | `HvigorfileRuntimeBridge`, `HvigorfileRuntimePlan`, `HvigorfileExecutionPolicy`, `HvigorfileRuntimeOutput`, `HvigorfileDiagnostic`, `HvigorRuntimeProcess` | `HvigorfileRuntimeBridge::new`, `discover_scripts`, `execute_project_script`, `execute_module_script`, `normalize_overlay`, `capture_hooks`, `capture_task_graph`, `emit_hvigor_equivalent_error` | new from hvigorfile/hvigorconfig startup gap | project/module `hvigorfile.ts`, `hvigorconfig.ts`, hvigor core script loading, `projectOhosConfigManager.getOverrides/getHooks`, `build-option-path-info.js::getHookFilePath` |
| `cmd/build/hvigor_hooks.rs` | `HvigorHookExecutor`, `HvigorHookStage`, `HvigorHookInvocation`, `HvigorHookResult` | `HvigorHookExecutor::new`, `run_before_stage`, `run_after_stage`, `run_registered_hook`, `merge_hook_outputs` | new from hvigorfile runtime output | hvigor hooks registered from project/module `hvigorfile.ts` |
| `cmd/build/hvigor_tasks.rs` | `HvigorCustomTaskExecutor`, `HvigorCustomTaskPlan`, `HvigorTaskRuntimeAction`, `HvigorTaskRunResult` | `HvigorCustomTaskExecutor::new`, `splice_into_runner`, `run_task_action`, `run_before_dependencies`, `run_after_dependencies`, `emit_task_events` | new from hvigorfile runtime output | custom task/plugin graph registered by hvigor project/module scripts |
| `cmd/build/plan.rs` | `BuildPlan`, `BuildPlanBuilder`, `ModuleBuildPlan`, `StagePlanSet` | `BuildPlanBuilder::new`, `build`, `plan_module`, `plan_profile`, `plan_resource`, `plan_bundle`, `plan_package`, `plan_sign` | current `module_plan.rs`, `custom_config.rs`, profile/resource/package hand assembly | `TaskCreator` task inputs |
| `cmd/build/plugins.rs` | `PluginFactory`, `PluginPlanSet`, `BuiltInPluginPlan`, `SdkPluginPlan` | `build_rolldown_plugins`, `ets_transform`, `ohos_resolve`, `module_meta`, `json5`, `duplicate_check`, `har_dedup`, `sdk_plugins`, `clean_config_delete_plan` | current bundler/plugin construction | `arkts-pack.js::getBuiltInPlugins`, SDK `compile_plugin.js` plugin array |
| `cmd/build/custom_plugins.rs` | `CustomCompilePluginPlan`, `CustomCompilePluginSource`, `CustomPluginRuntimeBridge`, `CustomPluginOrder`, `CustomPluginDiagnostic` | `CustomCompilePluginPlan::from_session`, `load_sources`, `validate_plugins`, `order_plugins`, `execute_hook`, `emit_hvigor_equivalent_error` | new, from `HvigorScriptOverlay`/build option fields once session exposes them | `ArkCompile.getHvigorPluginFile`, `arkts-pack.js::loadCustomCompilePlugins`, `validatePlugin`, `checkPluginDuplicateName`, `getPlugins` order `first/last/function` |
| `cmd/build/plugin_order.rs` | `PluginOrderPlanner`, `PluginOrderInput`, `PluginOrderResult`, `PluginNameIndex` | `PluginOrderPlanner::new`, `place_builtins`, `apply_custom_order`, `verify_unique_names`, `to_plugin_plan_set` | new from plugin assembly | `arkts-pack.js::getPlugins`, plugin name-to-index map passed to custom order function |
| `cmd/build/compile_env.rs` | `CompileEnvironmentPlanner`, `BundleOutputPlanner`, `ManualChunkPlanner`, `WatchConfigPlanner` | `CompileEnvironmentPlanner::from_module_plan`, `BundleOutputPlanner::from_module_plan`, `ManualChunkPlanner::from_module_plan`, `WatchConfigPlanner::from_mode_inventory` | current `compile_arkts_modules.rs`, cache/watch args | `arkts-pack.js::generateConfig`, `getCompileEnv`, `getOutputConfig`, `manualChunks`, `getWatchConfig` |
| `cmd/build/runner.rs` | `BuildRunner`, `DependencyWaveRunner`, `StageExecutor` | `run`, `run_wave`, `run_module`, `execute_stage`, `merge_diagnostics` | current top-level build orchestration | `TaskInitializer` dependency execution |
| `cmd/build/pipeline.rs` | `ModulePipeline`, `ModulePipelineOutput` | `run`, `run_profile`, `run_resource`, `run_bundle`, `run_obfuscation`, `run_ts2abc`, `run_package`, `run_sign` | current scattered build stage calls | HAP/HSP/HAR task chain |
| `cmd/build/stages.rs` | `ProfileStage`, `ResourceStage`, `BundleStage`, `Ts2abcStage`, `PackageStage`, `SignStage` | `execute` per stage | current stage modules | hvigor task wrappers |
| `cmd/build/native.rs` | `NativeStage`, `NativeCommandPlan`, `NativeStageOutput` | `configure_cmake`, `run_ninja`, `process_libs`, `strip_libs`, `cache_native_libs` | current native command/path orchestration | `native-task-initializer.js`, `BuildNativeWithCmake`, `BuildNativeWithNinja`, `ProcessLibs`, `DoNativeStrip`, `CacheNativeLibs` |
| `cmd/build/preview.rs` | `PreviewStage`, `PreviewBuildPlan`, `PreviewStageOutput` | `prepare_preview_profile`, `compile_preview_resource`, `compile_preview_arkts`, `update_preview_assets` | current preview helpers | `PreviewBuild`, `BuildPreviewerRes`, `PreviewArkTS`, `PreviewCompileResource`, `CopyPreviewProfile` |
| `cmd/build/test.rs` | `TestStage`, `TestBuildKind`, `TestStageOutput` | `generate_unit_test_template`, `generate_ohos_test_template`, `copy_mock_config`, `run_test_compile`, `write_unit_test_result` | current unit/ohosTest helpers | `UnitTestBuild`, `OhosTestArkCompile`, `GenerateUnitTestTemplate`, `GenerateOhosTestTemplate`, `GenerateUnitTestResult` |
| `cmd/build/reload.rs` | `ReloadStage`, `ReloadKind`, `ReloadPatchConfig` | `build_hot_reload_resource`, `compile_hot_reload_arkts`, `compile_cold_reload_arkts`, `collect_patch_importers` | current hot/cold reload path | `HotReloadBuild`, `BuildHotReloadResource`, `HotReloadArkTS`, `ColdReloadArkTS` |
| `cmd/build/coverage.rs` | `CoverageStage`, `CoverageInstrumentationPlan`, `CoverageSourceMapRewrite` | `instrument_sources`, `replace_sourcemap`, `before_generate_device_coverage`, `generate_device_coverage` | current coverage helpers | SDK `buildInstrument`, `ReplaceSourceMapInCoverageMode`, `GenerateDeviceCoverage` |
| `cmd/build/artifacts.rs` | `ArtifactReport`, `ArtifactSummary` | `from_outputs`, `print`, `output_paths` | current output summary | final artifact notes |
| `cmd/build/cache.rs` | `CliCachePlan`, `CacheStageAdapter` | `from_build_plan`, `restore`, `commit` | current `task_cache.rs`, `compile_arkts_cache.rs` | rollup/task cache |
| `cmd/build/parity_report.rs` | `HvigorParityReport`, `ParityGap`, `ParityGate` | `HvigorParityReport::from_stage_outputs`, `record_lifecycle_gap`, `record_custom_plugin_gap`, `fail_on_blockers` | new | `rolldown-oxc-parity-gap-audit.md` enforcement |

## 当前文件迁移表

| 当前文件 | 新位置/处理 |
| --- | --- |
| `module_plan.rs` | 迁入 `cmd/build/plan.rs` + `arkdown-session` context；删除旧文件 |
| `intermediates.rs` | 迁入 `arkdown-session::artifact_layout`；CLI 只消费 `ArtifactLayout`，删除旧文件 |
| `custom_config.rs` | 迁入 `arkdown-config` normalize + `session.rs`；删除旧文件 |
| `compile_arkts_modules.rs` | 迁入 `pipeline.rs::run_bundle` + `arkdown` `BundlePlan`；删除旧文件 |
| `compile_arkts_output.rs` | 迁入 `arkdown::output` + `artifacts.rs` 的正式 bundle output/stage output；删除旧文件 |
| `compile_arkts_abc.rs` | 迁入 `stages.rs::Ts2abcStage` + `arkdown-ts2abc::Ts2abcCompiler`；删除旧文件 |
| `compile_arkts_handoff.rs` | 删除；所有字段迁入 `Ts2abcPlan`、`AbcSourceMapPlan`、`AbcCachePlan`、`FileInfoManifestWriter` |
| `compile_arkts_package_entries.rs` | 迁入 `BuildPlanBuilder::plan_package` + `arkdown-pack::PackagePlanBuilder`；删除旧文件 |
| `compile_arkts_arkdata.rs` | 迁入 `ResourceStage`/package entry plan，输入来自 resource inventory；删除旧文件 |
| `compile_arkts_widget.rs` | 迁入 `Ts2abcPlanBuilder::with_widget_units` + `arkdown-abc::WidgetCompiler`；删除旧文件 |
| `compile_arkts_cache.rs` | 迁入 `arkdown-cache::AbcCachePlan` + `arkdown-ts2abc::cache`；删除旧文件 |
| `coverage_instrument.rs` | 迁入 `EtsTransformPlan` 或 dedicated `CoverageStage`；删除旧文件 |
| `generate_har_declarations.rs` | 迁入 `arkdown-ets-transform::declaration` + HAR package plan；删除旧文件 |
| `task_cache.rs` | 迁入 `arkdown-cache`；CLI 只做 stage adapter，删除旧文件 |
| native command helpers | 迁入 `native.rs` typed stage；domain crates 只消费 inventory/output |
| preview/unit/ohosTest/reload helpers | 迁入 `preview.rs`, `test.rs`, `reload.rs`, `coverage.rs` typed stages；删除散落 helper |

## SPEC-049 ABC/AOT 串联修正

当前分支已合并 SPEC-049，CLI 的 ArkTS ABC 阶段必须按以下目标迁移：

```text
BuildPlanBuilder::plan_bundle()
  -> BundlePlan / Transform facts
  -> BuildPlanBuilder::plan_ts2abc()
     -> Ts2abcPlan
  -> ModulePipeline::run_ts2abc()
     -> Ts2abcStage::execute()
        -> arkdown_ts2abc::Ts2abcCompiler::compile()
           -> ArkdownAotBackend -> arkdown_abc::aot::AotRunner
           -> or SdkEs2abcBackend
  -> PackageStage package entries
```

| 当前文件/函数 | 当前问题 | 目标 owner/API | 对应上游逻辑 |
| --- | --- | --- | --- |
| `compile_arkts_abc.rs` | CLI 当前直接组装 ABC 编译输入、cache、package entries，混合 stage policy 和 domain policy | `Ts2abcStage::execute`, `Ts2abcPlanBuilder::from_module_pipeline`, `PackageEntryPlanner::from_ts2abc_output` | hvigor `CompileArkTS` task 调 SDK genAbc / es2abc |
| `compile_arkts_handoff.rs::write_compile_arkts_abc_handoff` | handoff 文件是当前 Rust 内部桥，不是 hvigor production artifact | 删除；需要调试或 parity 时由 `Ts2abcPlan::to_parity_snapshot` 或正式 artifact owner 输出结构化快照，不能保留 handoff writer | Rust adapter，替代旧全局临时文件桥 |
| `compile_arkts_handoff.rs::read_compile_arkts_source_maps` | source map 读取在 CLI 中散落，容易绕过 source map policy | `Ts2abcSourceMapInput::from_bundle_output`, `AbcSourceMapPlan::from_bundle_output` | SDK `generate_sourcemap.js`, `preserveSourceMap` |
| `compile_arkts_widget.rs` | widget ABC 仍在 CLI 阶段直接拼接 | `Ts2abcPlanBuilder::with_widget_units`, `arkdown_abc::WidgetCompiler` | hvigor widget compile path |
| `compile_arkts_package_entries.rs` | package entries 从 CLI 临时推导 | `PackageEntryPlanner::from_ts2abc_output`, `PackagePlanBuilder::add_abc_outputs` | `MakePackInfo` / HAP-HSP-HAR package staging |

未实现或未对齐项必须进入 CLI 阶段计划：

1. `BuildPlanBuilder::plan_ts2abc` 必须消费 loader json、bundle chunk graph、transform facts、source map policy、bytecode HAR/AOT/cache facts，不能只消费当前 handoff 文件。
2. `Ts2abcStage` 必须选择 Rust AOT backend 或 SDK backend；Rust backend 必须调用 `arkdown-abc::aot::AotRunner`，不能走旧 source-to-final facade。
3. `compile_arkts_handoff.rs` 在 typed plan 迁移完成后必须删除；同类 handoff 文件不得以 debug/parity 名义保留。
4. `compile_arkts_output.rs`、`compile_arkts_package_entries.rs`、`compile_arkts_cache.rs`、`compile_arkts_widget.rs` 这类当前承担跨阶段数据搬运的文件也必须拆成正式 owner 后删除。
5. package/sign 阶段只能消费 `Ts2abcOutput` 和 `PackagePlan`，不得重新扫描 ABC output 目录猜 entry。
