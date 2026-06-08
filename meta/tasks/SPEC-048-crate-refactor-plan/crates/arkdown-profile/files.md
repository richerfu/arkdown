# File API Plan: crates/arkdown-profile

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 现有 profile crate 中大量函数直接读 dependency/package/profile 文件；这些读取必须迁到 `arkdown-session::{ProfileStore,DependencyGraphSnapshot,SourceStore}`。
- `dependency_graph.rs` 整体迁出到 `arkdown-session::dependency_graph`，profile 只消费 graph view。
- `loader_json/runtime_only.rs`、`bytecode_har.rs` 中的扫描逻辑拆分：扫描和依赖分类归 session，loader 字段组装保留 profile。
- `GenerateLoaderJson` 不是简单 entry writer：dynamic import、startup、dependency ets folder、ByteCode HAR、worker/card/form/declaration/other compile files 都必须有显式 plan 字段。
- HAR `oh-package.json` 的生成必须覆盖 artifactType、worker metadata、runtimeOnly dynamic import、resource metadata、bytecode/interface HAR 分支。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `ProfileProcessor`, `ProfileProcessPlan`, `ProfileProcessOutput` | `ProfileProcessor::new(plan).process()`, `ProfileProcessor::write_loader_json()`, `ProfileProcessor::write_pkg_context_info()`, `ProfileProcessor::write_package_module_json()` | 当前 `ProfileProcessor` 保留并改为 plan-based | `ProcessProfile`/`GenerateLoaderJson`/`GeneratePkgContextInfo` typed facade |
| `plan.rs` | `ProfileProcessPlan`, `LoaderJsonPlan`, `PkgContextInfoPlan`, `PackageModuleJsonPlan`, `ProfileArtifactPaths` | `from_context_input`, `validate`, `module_json_input`, `loader_input` | 新增，从 CLI/profile long params 汇总 | hvigor target task inputs |
| `writer.rs` | `JsonWriter`, `JsonWriteStyle`, `StableObjectOrder` | `write_json`, `write_json5_compatible`, `write_minified`, `order_fields` | `module_json_writer.rs`, `packager.rs` JSON order helpers | hvigor JSON artifact order |
| `generated_build_profile.rs` | `GeneratedBuildProfileInput`, `GeneratedBuildProfileOutput`, `BuildProfileGenerator`, `HarBuildProfileGenerator` | `generate_build_profile`, `generate_har_build_profile`, `write_generated_profile`, `dependency_profile_inputs` | new from CLI build profile generation and current profile helpers | `BuildProfileTask`, `HarBuildProfileTask`, `GeneratedBuildProfileCA` |
| `merge_profile.rs` | `MergeProfileProcessor`, `MergeProfileInput`, `MergeProfileOutput` | `merge`, `merge_app_scope`, `merge_module_profile`, `apply_target_overrides` | 当前 `module_json_writer.rs::write_module_json` 部分 | `merge-profile.js`, `abstract-merge-profile.js` |
| `process_profile.rs` | `ProcessProfileInput`, `ProcessProfileOutput` | `validate_compile_mode`, `process_profile`, `process_profile_with_ark_vm_version`, `ark_virtual_machine_from_version` | 当前同名函数保留，参数改为 plan | `process-profile.js` |
| `module_json_writer.rs` | `ModuleJsonWriter`, `TargetAbilityOverride`, `DependencyProfileMerge` | `write_module_json`, `ModuleJsonWriter::write`, `apply_dependency_profiles`, `append_metadata` | 当前同名文件保留，删除内部 dependency file reads | `MergeProfile`, `GeneratePkgModuleJson` |
| `pkg_module_json.rs` | `PkgModuleJsonInput`, `PkgModuleJsonOutput`, `PkgModuleJsonWriter` | `from_processed_profile`, `apply_package_overrides`, `write_pkg_module_json` | split from current module json/package entry assembly | `generate-pkg-module-json.js` |
| `loader_json.rs` | `LoaderJson`, `AotLoaderFields`, `ByteCodeHarEntry`, `LoaderJsonWriter`, `LoaderCompileEntryWriter`, `LoaderDynamicImportWriter`, `LoaderStartupWriter` | `LoaderJson::from_plan`, `to_value`, `write`, `collect_compile_entries_from_plan`, `collect_dynamic_import_inputs`, `collect_startup_inputs`, `collect_dependency_ets_folder_entries`, `aot_fields` | 当前 `LoaderJson` 和 tests 保留；path helpers 迁 `arkdown-path` | `generate-loader-json.js::collectCompileFiles/declareDynamicImportInputs/addStartupInputFiles/collectDepEtsFolderFilesEntry`, `abstract-generate-loader-json.js` |
| `loader_json/runtime_only.rs` | `RuntimeOnlyLists`, `RuntimeOnlyLoaderFields` | `collect_dynamic_import_dep_names_from_graph`, `collect_runtime_only_source_files_from_store` | 当前 file read functions 改成 graph/source store view | `runtimeOnly`, `dynamicDependencies` loader fields |
| `loader_json/bytecode.rs` | `LoaderByteCodeHarFields`, `LoaderByteCodeHarDependency`, `LoaderOtherCompileFileSet`, `LoaderHostModuleInfo` | `from_bytecode_har_view`, `collect_other_compile_files`, `collect_host_module_info`, `check_bytecode_har_constraints`, `write_loader_fields` | split from current `bytecode_har.rs` and loader writer | `GenerateLoaderJson::setByteCodeHarInfoAndOtherCompileFiles`, `checkByteCodeHar`, `ProcessOhPackageJson::processByteCodeHar` |
| `loader_json/workers.rs` | `WorkerLoaderEntry`, `WorkerLoaderWriter`, `WorkerSourceRewritePolicy` | `from_source_store`, `write_worker_entries`, `rewrite_worker_extension_for_bytecode_or_obf`, `dedupe_workers` | current CLI worker scans + profile loader assembly | SDK `main.js::loadWorker/readWorkerFile/filterWorker`, `process-oh-package-json.js::addWorkerData/processWorkerConfig` |
| `pkg_context_info.rs` | `PkgContextInfo`, `PkgContextInfoEntry`, `SoContextInfo` | `PkgContextInfo::from_plan`, `default_so_context`, `write` | 从 `generate-pkg-context-info` 逻辑新增 | `generate-pkg-context-info.js`, `soLibsPattern` |
| `router_map.rs` | `RouterMapEntry`, `RouterMapProcessor`, `RouterMapOutput` | `collect_router_entries`, `RouterMapProcessor::write`, `fingerprint_inputs` | 当前 `collect_router_entries`; plugin-router-map 迁入 | `process-router-map.js`, `router-map-plugin.js` |
| `navigation.rs` | `NavigationConfigInput`, `NavigationRouteEntry`, `NavigationConfigProcessor`, `BuildFunctionRoute` | `load_navigation_config`, `route_entries`, `build_function_entries`, `write_navigation_artifact` | new from SDK project config initialization | SDK `main.js::loadNavigationConfig`, routerMap `pageSourceFile/name/buildFunction` handling |
| `process_share_config.rs` | `ShareConfigInput`, `ShareConfigOutput`, `ShareConfigProcessor` | `merge_share_config`, `write_share_config`, `dependency_share_entries_from_graph` | 当前同名函数保留，dependency reads 迁 session | `process-share-config.js` |
| `process_startup_config.rs` | `StartupConfigInput`, `StartupConfigOutput`, `StartupConfigProcessor` | `merge_startup_config`, `write_startup_config`, `startup_tasks_from_graph` | 当前同名函数保留，dependency package reads 迁 session | `process-startup-config.js` |
| `startup_validation.rs` | `StartupTaskDependencyValidator`, `StartupTaskGraph`, `StartupValidationDiagnostic` | `StartupTaskDependencyValidator::from_plan`, `validate_dependencies`, `all_task_names`, `check_app_preload_hint_src_entry` | new | `CompileResource::checkStartupTaskDependencies/validateDependencies/getAllStartupTaskNames`, `PackageHar::checkAppPreloadHintStartupTasksSrcEntry` |
| `process_privacy_profile.rs` | `PrivacyProfileInput`, `PrivacyProfileOutput`, `ProjectPrivacyProfileInput`, `ProjectPrivacyProfileOutput`, `PrivacyProfileProcessor` | `process_module_privacy_profile`, `process_project_privacy_profile`, `privacy_profile_is_enabled`, `merge_pac_values` | 当前保留；`collect_dependency_package_dirs` 迁 session | `pac-process/process-*-privacy-profile.js` |
| `process_permission_reason.rs` | `PermissionReasonResourceInput`, `DependencyPermissionReasonResourceInput`, `PermissionReasonProcessor` | `process_permission_reason_resources`, `merge_dependency_permission_reason_resources` | 当前保留；dependency roots 由 plan 给 | permission reason resource merge |
| `process_shortcut_profile.rs` | `ShortcutProfileInput`, `ShortcutProfileProcessor` | `process_shortcut_profiles`, `rewrite_shortcut_bundle_name` | 当前保留；file read 来自 ProfileStore | `process-shortcut-profile.js` |
| `process_distro_filter.rs` | `DistroFilterProfileInput`, `DistroFilterProfileOutput`, `DistroFilterPlan`, `DistroFilterProcessor` | `process_distro_filter_profile`, `build_distro_filter_plan`, `append_profile_metadata_to_module_json` | 当前保留；profile file discovery 迁 session | `process-distro-filter.js` |
| `syscap.rs` | `SyscapInput`, `SyscapOutput`, `SyscapTransformer`, `SyscapPrechecker` | `precheck_syscap`, `transform_syscap`, `write_rpcid_sc`, `validate_device_syscap` | new from current syscap/profile handling | `pre-check-syscap.js`, `syscap-transform.js`, `abstract-syscap-transform.js` |
| `metadata.rs` | `MetadataInput`, `MetadataOutput`, `MetadataGenerator`, `DistroFilterMetadata` | `generate_metadata`, `related_entry_metadata`, `distro_filter_obj`, `write_metadata` | new from metadata/package helpers | `generate-metadata.js`, `abstract-generate-metadata.js` |
| `module_info.rs` | `ModuleInfoInput`, `ModuleInfoOutput`, `ModuleInfoWriter`, `HarModuleInfoWriter` | `create_module_info`, `create_har_module_info`, `bytecode_har_module_info`, `write_module_info` | new from module info generation and module meta facts | `CreateModuleInfo`, `CreateHarModuleInfo`, `module-meta-info-plugin.js` |
| `form_profile.rs` | `FormProfileInput`, `ModuleFormProfileInput`, `FormProfileEntry`, `FormSourceEntry`, `FormProfileProcessor` | `collect_form_profiles`, `collect_form_sources`, `collect_module_form_sources`, validation helpers | 当前保留；module json/package reads 迁 session | form profile compile entry collection |
| `har_abilities.rs` | `HarAbility`, `HarAbilityCollector` | `collect_har_abilities` | 当前保留 | HAR source ability metadata |
| `insight_intent.rs` | `InsightIntentProfileInput`, `InsightIntentSrcEntry`, `InsightIntentProcessor` | `collect_insight_intents`, `rewrite_src_entry_for_har`, `write_insight_metadata` | new | SDK `userIntents_parser/*`, `ProcessHarArtifacts::processInsightIntent` |
| `bytecode_har.rs` | `ByteCodeHarScanEntry`, `DynamicImportLibEntry`, `HostModuleInfo`, `ByteCodeHarMetadataWriter` | `scan_bytecode_har_deps_from_graph`, `collect_bytecode_har_update_version_info`, `collect_bytecode_har_other_compile_files`, `collect_dynamic_import_lib_info`, `startup_task_compile_entries`, `insight_intent_compile_entries` | 当前 graph scanner 拆：graph scan 迁 session，field writer 保留 | ByteCode HAR loader metadata, `ProcessHarArtifacts::collectByteCodeHarStartupTasks/collectByteCodeHarInsightIntentSrcEntry` |
| `oh_package_writer.rs` | `GeneratedOhPackageJson`, `OhPackageWriter`, `OhPackageWorkerMetadata`, `OhPackageRuntimeOnlyMetadata`, `OhPackageResourceMetadata`, `OhPackageArtifactType` | `write_har_oh_package`, `apply_dynamic_import`, `apply_bytecode_har`, `apply_interface_har_warning`, `apply_resource_metadata`, `apply_worker_metadata`, `add_artifact_type` | 从 HAR `process-oh-package-json` 新增 | `har/process-oh-package-json.js::addArtifactType/processByteCodeHar/processInterfaceHar/addWorkerData/processWorkerConfig/addDynamicImport/processResourceConfig` |
| `validate_permissions.rs` | `RequestPermissionValidationInput`, `SdkPermissionData`, `PermissionValidator` | `validate_request_permissions`, `collect_sdk_permission_data`, `collect_sdk_permission_names` | 当前保留；SDK files 由 `SdkSnapshot` 给 | permission validation |

## 必迁/必删

| 当前文件/API | 处理 |
| --- | --- |
| `dependency_graph.rs::{LocalDependencyGraph,DependencyNode}` | 迁到 `arkdown-session::dependency_graph` |
| `loader_json/runtime_only.rs` 中 `read_*`, `resolve_*`, `collect_declared_dependency_roots` | 迁到 session graph/source inventory |
| `bytecode_har.rs::ByteCodeHarGraphScanner` | 迁到 session dependency graph |
| `to_absolute_path_string`, `normalize_path_string` | 迁到 `arkdown-path::serializer` |
