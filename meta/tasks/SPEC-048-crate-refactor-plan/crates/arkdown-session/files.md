# File API Plan: crates/arkdown-session

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 新 crate 必须从第一次实现就读真实输入，不允许空 snapshot。
- 迁移来源主要是 `arkdown-cli/src/cmd/build/*`、`arkdown-profile::{dependency_graph,loader_json,runtime_only,bytecode_har}`、`arkdown-config::resolve/sdk`、`arkdown-resolver` package indexes。
- 对应 hvigor `PluginFactory + TaskInitializer + TargetTaskService + DependencyManager + PathInfo` 的 Rust session 化。
- `hvigorfile.ts`/`hvigorconfig.ts` 的发现、fingerprint、normalized overlay、hook graph、自定义 task graph 必须进入 session；CLI 负责执行 JS/TS runtime bridge，但 session 必须记录它对后续 plan 和 runner DAG 的输入影响。
- `SdkSnapshot` 必须覆盖上游 `DefaultSdk` tool/schema/permission/compile plugin getter；不能只保留泛化 `tool_path/schema_path`。
- `DependencyGraphSnapshot` 必须覆盖 `TaskService` 的分类 view，包括 remote HSP/HAR、ByteCode HAR、SO deps、direct/all/root deps、HSP recursion、dynamic HSP 和 affects compilation dependencies。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | `BuildSession`, `BuildSessionBuilder`, `ModuleCompileContext` exports | `BuildSession::module`, `modules`, `dependency_graph`, `profile_store` | 新 crate exports | session boundary |
| `builder.rs` | `BuildSessionBuilder`, `BuildSessionBuildInput`, `ParallelInventoryBuilder` | `new`, `with_project_root`, `with_target`, `build`, `read_project_inputs`, `read_modules`, `read_packages`, `build_inventories` | CLI build discovery, config load, profile/resource scans | `PluginFactory.getAppPlugin`, `TaskInitializer.initialize*` |
| `hvigor_script.rs` | `HvigorScriptSnapshot`, `HvigorScriptFile`, `HvigorScriptKind`, `HvigorScriptOverlay`, `HvigorHookInput`, `HvigorBuildOptionOverride`, `HvigorScriptFingerprint`, `HvigorScriptRuntimeEffect` | `HvigorScriptSnapshot::discover`, `apply_overlay_to_project`, `apply_overlay_to_module`, `compile_plugin_file`, `hook_inputs`, `task_graph`, `fingerprint`, `runtime_effects` | new from hvigorfile/hvigorconfig startup gap | project/module `hvigorfile.ts` and `hvigorconfig.ts`, `projectOhosConfigManager.getOverrides/getHooks`, `build-option-path-info.js::getHookFilePath` |
| `hvigor_task_graph.rs` | `HvigorTaskGraphSnapshot`, `HvigorTaskNode`, `HvigorTaskEdge`, `HvigorTaskActionRef`, `HvigorHookGraphSnapshot`, `HvigorHookPoint` | `HvigorTaskGraphSnapshot::from_runtime`, `ordered_nodes`, `edges`, `actions_for_stage`, `hooks_for_stage`, `fingerprint` | new from hvigorfile runtime bridge | hvigor custom task/plugin graph and hook registration produced by project/module `hvigorfile.ts` |
| `project.rs` | `ProjectSnapshot`, `ProjectPaths`, `SelectedProduct`, `SigningInputSet` | `ProjectSnapshot::from_raw`, `module_ids`, `product`, `signing_inputs` | `arkdown-cli` build args/config, `arkdown-config` normalized output | project model/global data service |
| `module.rs` | `ModuleSnapshot`, `ModuleSourceRoots`, `ModuleProfileRefs`, `ModuleTargetSnapshot` | `ModuleSnapshot::from_raw`, `source_roots`, `resource_roots`, `profile_refs`, `package_manifest` | CLI/profile direct module reads | module model/target data |
| `package.rs` | `PackageSnapshot`, `PackageKind`, `PackageSource`, `PackageManifestFingerprint` | `PackageSnapshot::from_ohpm_result`, `from_local_module`, `manifest`, `root` | profile dependency graph package reads | dependency manager package records |
| `dependency_graph.rs` | `DependencyGraphSnapshot`, `DependencyNode`, `DependencyEdge`, `DependencyClass`, `DependencyGraphBuilder`, `DependencyWave`, `DependencyQueryView` | `build`, `node`, `edges_from`, `topo_waves`, `har_dependencies`, `hsp_dependencies`, `bytecode_hars`, `runtime_only_packages`, `native_lib_owners`, `direct_dependencies`, `all_dependencies`, `root_dependencies`, `affects_compilation_dependencies` | `arkdown-profile::dependency_graph`, loader_json runtime-only/bytecode scanners | `src/project/dependency/dependency-manager.js`, `module-dependency-info.js`, `TaskService.getDirectDependencies/getAllDependencies/getAffectsCompilationDependencies` |
| `dependency_views.rs` | `HarDependencyView`, `HspDependencyView`, `ByteCodeHarDependencyView`, `RemoteHspDependencyView`, `NativeSoDependencyView`, `DynamicDependencyView` | `from_graph`, `remote_hars`, `remote_hsps`, `hsp_recursion`, `hsp_or_bytecode_har_dependencies`, `so_dependencies`, `dynamic_dependencies`, `dependency_name_to_root_path`, `dependency_name_to_dep_info` | split from current dependency graph readers | `TaskService.getRemoteHarDependencies`, `getOhpmRemoteHspDependencies`, `getHspRecursion`, `getHspOrByteCodeHarDependencies`, `getSODependencies`, `getDependencyName2RootPath`, `getDependencyName2DepInfo` |
| `resolver_index.rs` | `ResolverIndexBuilder`, `ResolverIndexSnapshot` | `build_for_module`, `package_index`, `sdk_index`, `source_root_index` | resolver package root index, plugin resolve config | `arkts-pack.js::getResolveModules` |
| `profile_store.rs` | `ProfileStore`, `ProfileFile`, `ParsedProfile`, `ProfileRef`, `RouterMapInventory`, `StartupInventory`, `ShareInventory` | `discover`, `profile_by_ref`, `router_maps_for_module`, `startup_for_module`, `fingerprint` | `arkdown-profile` profile reads/scans | `ProcessRouterMap`, `ProcessShareConfig`, `ProcessStartupConfig` |
| `sdk.rs` | `SdkSnapshot`, `SdkFileId`, `SdkComponent`, `SdkCompilePluginInfo`, `SdkToolchainSnapshot`, `SdkSchemaSnapshot`, `SdkPermissionSnapshot`, `SdkCompilePluginSnapshot`, `SdkAotSnapshot` | `SdkSnapshot::from_config`, `tool_path`, `schema_path`, `support_extensions`, `compile_plugin`, `permission_names`, `aot_config`, `fingerprints` | `arkdown-config::sdk`, plugin transform SDK reads | `src/sdk/*.js`, `load-sdk-functions.js`, `DefaultSdk` getter family |
| `sdk_tools.rs` | `SdkToolKind`, `SdkToolPath`, `SdkToolResolver` | `restool`, `package_tool`, `hap_sign_tool_v2`, `hap_to_bin`, `syscap_tool`, `verify_sign_config_tool`, `cmake_tool`, `native_ninja_tool`, `llvm_strip`, `llvm_read_elf` | split from `arkdown-config::sdk` path helpers | `DefaultSdk.getRestool/getPackageTool/getHapSignToolV2/getHapTobin/getSysCapTool/getVerifySignConfigTool/getCmakeTool/getNativeNinjaTool/getSdkLlvmStrip/getSdkLlvmReadElf` |
| `sdk_schemas.rs` | `SdkSchemaKind`, `SdkSchemaPath`, `SdkSchemaResolver` | `app_schema`, `module_schema`, `page_schema`, `form_schema`, `router_map_schema`, `share_config_schema`, `startup_schema`, `syscap_schema`, `pac_schema`, `shortcuts_schema`, `insight_intent_schema` | split from SDK snapshot | `DefaultSdk.getAppSchema/getModuleSchema/getPageSchema/getFormSchema/getRouterMapSchema/getShareConfigSchema/getAppStartupSchema/getHarStartupSchema/getHspStartupSchema/getSysCapSchema/getPacSchema/getShortcutsJsonSchema/getInsightIntentSchema` |
| `sdk_compile_plugin.rs` | `SdkCompilePluginSnapshot`, `EtsLoaderPath`, `JsLoaderPath`, `SdkArkTsFeatureGate`, `SdkSupportExtensionSet` | `ets_loader`, `js_loader`, `ark_version`, `arkui_x_dir`, `sdk_ets_dir`, `sdk_js_dir`, `component_metadata`, `support_extensions` | current SDK/plugin transform reads | `DefaultSdk.getEtsLoader/getJsLoader/getArkVersion/getArkUIXSdkDir/getSdkEtsDir/getSdkJsDir/getEtsComponentVersion/getEtsComponentReleaseType` |
| `resource_inventory.rs` | `ResourceInventory`, `ResourceRoot`, `ResourceInput`, `NativeLibInventory` | `discover`, `resources_for_module`, `native_libs_for_package`, `fingerprint` | `arkdown-resource::scan`, CLI/resource path logic | `CompileResource`, `ProcessLibs` |
| `native_inventory.rs` | `NativeBuildInventory`, `NativeBuildTarget`, `NativeAbi`, `NativeLibCandidate`, `NativeDebugSymbolPolicy` | `discover`, `cmake_targets`, `ninja_outputs`, `stripped_libs`, `debug_symbol_inputs`, `fingerprint` | CLI native path logic, pack native scans | `native-task-initializer.js`, `ProcessLibs`, `DoNativeStrip`, `CacheNativeLibs`, `CollectDebugSymbol` |
| `mode_inventory.rs` | `BuildModeInventory`, `PreviewInventory`, `TestInventory`, `ReloadInventory`, `CoverageInventory` | `from_project`, `preview_inputs`, `unit_test_inputs`, `ohos_test_inputs`, `reload_patch_config`, `coverage_inputs` | current CLI preview/test/coverage config reads | `PreviewBuild`, `UnitTestBuild`, `HotReloadBuild`, `GenerateDeviceCoverage` |
| `source_store.rs` | `SourceStore`, `SourceFile`, `SourceContent`, `SourceFingerprint`, `CodeResourceInventory`, `VisualSourceInventory`, `WorkerSourceInventory` | `discover`, `read_text`, `read_bytes`, `source_by_path`, `fingerprint`, `code_resources_for_module`, `visual_files_for_source`, `worker_entries_for_module` | bundler/plugin source reads, runtime-only file scans | `arkts-pack.js` input object/source inventories, `copy_resources_util.js`, SDK `loadWorker/readWorkerFile`, `rollup-plugin-visual.js` |
| `artifact_layout.rs` | `ArtifactLayout`, `ModuleArtifactLayout`, `StageArtifactLayout` | `for_module`, `loader_json_path`, `pkg_context_path`, `intermediates_res`, `package_output`, `signed_output`, `source_map_dir` | CLI path derivation, profile/resource/pack late path assembly | hvigor `PathInfo`, `BuildDirConst` |
| `fingerprint.rs` | `InputFingerprintSet`, `InputFingerprint`, `FingerprintKind`, `FingerprintBuilder` | `build`, `add_file`, `add_value`, `for_stage`, `changed_since`, `explain_change` | `arkdown-cache`, invalid-cache scattered hashes | `invalid-cache-plugin.js` |
| `context.rs` | `ModuleCompileContext`, `DependencyView`, `ArtifactView`, `FingerprintView` | `new`, `module`, `target`, `dependencies`, `artifacts`, `resolver_index`, `profile_store` | CLI long argument chains | target task service context |
| `plan_input.rs` | `ProfilePlanInput`, `ResourcePlanInput`, `BundlePlanInput`, `PackagePlanInput`, `SignPlanInput` | `from_context` per stage | new narrow views | `TaskCreator` target task inputs |
| `diagnostics.rs` | `SessionDiagnostic`, `SessionDiagnosticCollector` | `push`, `extend`, `emit_sorted` | scattered startup errors | hvigor logger/event aggregation |

## 强制迁移清单

| 当前来源 | 迁入文件 |
| --- | --- |
| `arkdown-profile/src/dependency_graph.rs` | `dependency_graph.rs`, `package.rs` |
| `arkdown-profile/src/loader_json/runtime_only.rs` scanning | `dependency_graph.rs`, `source_store.rs` |
| `arkdown-profile/src/bytecode_har.rs` graph scanning | `dependency_graph.rs` |
| CLI hand-written output/intermediate paths | `artifact_layout.rs` |
| plugin package/profile/SDK reads | `profile_store.rs`, `sdk.rs`, `source_store.rs` |
| CLI/native package scans | `native_inventory.rs`, `resource_inventory.rs` |
| preview/unit/ohosTest/reload ad hoc config reads | `mode_inventory.rs`, `profile_store.rs`, `source_store.rs` |
