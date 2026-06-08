# File API Plan: crates/arkdown

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `bundler.rs` 内 private `BundlePlan` 要变成 public `plan.rs::BundlePlan`。
- 当前 `Bundler` 构造 plugin 的行为必须移到 CLI；本 crate 只接收 rolldown plugin trait objects。
- `ModuleIdCollectorPlugin` 这类内部 plugin 如果只是 bundler fact collection，可保留为 internal fact plugin；不能依赖 concrete arkdown plugin crate。
- 本 crate 必须补齐 hvigor Rollup lifecycle 语义；Rolldown 没有原生等价 hook 的行为要由 pre/post stage bridge 或上游等价 runtime/toolchain 委托承担，不能让插件分支静默丢失。
- `BundlePlan` 必须覆盖 `arkts-pack.js::generateConfig/getOutputConfig/getCompileEnv/manualChunks/getWatchConfig` 的可观察字段；不能只保存 input/output/plugin 三项。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `Bundler`, `BundlePlan`, `BundleOutput` | `Bundler::new(plan).bundle()` | current exports narrowed | `arkts-pack.js::build`/Rollup typed facade |
| `plan.rs` | `BundlePlan`, `BundleInput`, `BundleOutputOptions`, `BundlePluginSet`, `BundleCachePolicy`, `SourceMapPolicy`, `LoaderResourceJoin` | `BundlePlan::validate`, `entry_inputs`, `output_options`, `plugins`, `loader_json`, `resource_outputs` | current `bundler.rs::BundlePlan`, CLI module plan | `arkts-pack.js::generateConfig`, `ArkCompile` joining `GenerateLoaderJson` and `CompileResource` |
| `bundler.rs` | `Bundler`, `BundleOutput`, `BundleDiagnostic`, internal `ModuleIdCollectorPlugin`, `ParsedModule` | `Bundler::new`, `bundle`, `run_rolldown`, `collect_module_ids`, `convert_output` | current `bundler.rs` | `runHvigorCompile`, rollup build/write |
| `compile_entry.rs` | `CompileEntry`, `CompileEntryMap`, `EntryKind` | `from_loader_plan`, `filter_declarations`, `to_rolldown_input` | current `compile_entry.rs` | `arkts-pack.js::getCompileInputObject` |
| `compile_env.rs` | `CompileEnvironment`, `CompileEnvFlagSet`, `CompileMode`, `BuildMode`, `FastBuildFeatureFlags` | `CompileEnvironment::from_plan`, `is_module_mode`, `should_emit_js`, `use_source_hash`, `use_path_placeholder`, `cache_key_material` | current CLI/plugin ad hoc env assembly | `arkts-pack.js::getCompileEnv`, `generateConfig.compileEnv` |
| `plugin_lifecycle.rs` | `RolldownLifecycleBridge`, `LifecycleHook`, `LifecycleParityReport`, `PluginSharedState`, `PluginHookContext`, `HookOrderPlan` | `RolldownLifecycleBridge::new`, `run_build_start`, `run_resolve_id`, `run_load`, `run_transform`, `run_module_parsed`, `run_before_build_end`, `run_build_end`, `run_after_build_end`, `run_clean_up`, `parity_report` | new Rust adapter around current plugin calls | `arkts-pack.js::doWrapper`, `wrapperBuildStart`, `wrapperResolveId`, `wrapperLoad`, `wrapperTransform`, `wrapperModuleParsed`, `wrapperBeforeBuildEnd`, `wrapperBuildEnd`, SDK `afterBuildEnd/cleanUp` |
| `plugin_context.rs` | `CompilePluginContext`, `PluginContextShare`, `PluginModuleInfoView`, `PluginFileNameResolver` | `get_file_name`, `get_module_ids`, `get_module_info`, `parse`, `share`, `record_diagnostic` | new typed replacement for Rollup context use | `arkts-pack.js::getCompilePluginContext`, `doWrapper` context object |
| `manual_chunks.rs` | `ManualChunkPolicy`, `ManualChunkDecision`, `ChunkName` | `ManualChunkPolicy::from_plan`, `chunk_for_module`, `is_common_dependency`, `is_vendor_dependency` | current bundle output naming rules | `arkts-pack.js::manualChunks` |
| `watch_config.rs` | `WatchConfigBridge`, `WatchInvalidationEvent`, `WatchParityPolicy` | `WatchConfigBridge::from_plan`, `event_for_changed_file`, `should_skip_build_end`, `to_watcher_inputs` | current watcher/cache glue | `arkts-pack.js::getWatchConfig`, `rollup-plugin-watch-change.js` |
| `output.rs` | `WrittenOutput`, `SourceMapOptions`, `ChunkSourceMap`, `BundleWrittenOutput` | `WrittenOutput::from_rolldown`, `write_to_layout`, `source_map_options`, `hvigor_source_map_scope` | current `output.rs` | `arkts-pack.js::getOutputConfig`, sourcemap path transform |
| `facts.rs` | `BundleFacts`, `ModuleFact`, `ImporteeFact`, `HarDedupFact`, `DuplicateOutputFact` | `merge_plugin_facts`, `facts_by_module`, `diagnostics` | new, consumes plugin outputs | `collect-importers-plugin`, module meta, HAR dedup |
| `diagnostics.rs` | `BundlerDiagnostic`, `RolldownErrorKind` | `from_rolldown_error`, `from_plugin_error` | new | `getRollupErrorMessage` |
| `cache_bridge.rs` | `BundleCacheBridge`, `ShouldInvalidCacheRequest`, `ShouldInvalidCacheResult` | `BundleCacheBridge::from_plan`, `ask_plugins`, `restore_rollup_cache`, `commit_rollup_cache` | current task/cache glue | `invalid-cache-plugin.js`, `rollup-cache-service.js`, `wrapperShouldInvalidCache` |

## 必迁/必改

- `arkdown` Cargo deps 删除 `arkdown-plugin-*` 和 `arkdown-profile`。
- `BundlePlan` 中所有 paths 已由 `arkdown-session::ArtifactLayout` 和 `arkdown-path` 准备好。
- `BundlePlan` 必须显式接收 loader_json 与 resource outputs；二者在上游均为 `ArkCompile` 依赖，但 `GenerateLoaderJson` 不依赖 `CompileResource`。
- `LifecycleParityReport` 必须列出每个上游 hook 在 Rolldown 中的执行点、pre/post stage bridge 或上游等价 runtime/toolchain 委托，作为 Phase 4 退出条件。
