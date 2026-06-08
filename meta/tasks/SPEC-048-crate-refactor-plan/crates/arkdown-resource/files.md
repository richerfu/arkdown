# File API Plan: crates/arkdown-resource

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `ResourceCompiler` 可以保留，但 input 改为 `ResourceCompilePlan`。
- 扫描从 `scan_resources(resources_dir)` 改为消费 session resource inventory；只允许 compiler stage 流式读取资源 bytes。
- restool config/command 对应 hvigor `RestoolCommandBuilder` 和 `RestoolConfigBuilder`。
- `ArkCompile.copyCodeResource` 复制的是源码根下非代码资源，不等同于 restool 资源；必须单独建 stage，不得混在 `compiler.rs::copy_resources`。
- `CompileResource` 会处理 router/share/startup/distro filter，并校验 startup task dependency；这些 profile artifacts 由 profile crate 写，resource crate 消费并传给 restool。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `ResourceCompiler`, `ResourceCompilePlan`, `ResourceCompileOutput` | `ResourceCompiler::new(plan).compile()` | current exports narrowed | `ProcessResource`/`CompileResource` typed facade |
| `error.rs` | `ResourceError`, `ResourceDiagnostic` | `ResourceError::scan`, `compile`, `duplicate_destination` | 当前 `error.rs` | restool/process resource errors |
| `plan.rs` | `ResourceCompilePlan`, `ResourceInputRoot`, `ResourceDependencyInput`, `ResourceOutputPaths`, `ResourceCompileOptions` | `ResourceCompilePlan::validate`, `restool_config_input`, `output_layout` | 新增，从 `ResourceCompileConfig/Input` 收敛 | hvigor task declared inputs |
| `code_resource.rs` | `CodeResourceCopyPlan`, `CodeResourceInput`, `CodeResourceDestination`, `CodeResourceFilter`, `CodeResourceCopier` | `CodeResourceCopyPlan::from_inventory`, `CodeResourceFilter::allows`, `CodeResourceCopier::copy`, `relative_destination_for`, `copied_entries` | new from CLI/source resource copy gaps | `copy_resources_util.js::getCopyOptions`, `ArkCompile.copyCodeResource`, excludes `.ets/.ts/.js` and user excludes |
| `preview_resource.rs` | `PreviewResourcePlan`, `PreviewAppResourceInput`, `PreviewResourceCopier`, `PreviewResourceOutput` | `PreviewResourcePlan::from_mode_inventory`, `copy_preview_resources`, `copy_preview_app_resources`, `write_preview_resource_index` | current preview helpers | `generatePreviewResourceFiles`, `PreviewBuild`, `BuildPreviewerRes`, preview app resource branch in copy options |
| `scan.rs` | `Qualifier`, `ResourceCategory`, `ResourceFile`, `ResourceInventoryView` | `scan_resources_from_inventory`, `classify_resource_file`, `qualifier_from_path` | 当前 `scan_resources` 改为 inventory view | resource root scan |
| `layout.rs` | `ResourceOutputLayout` | `new`, `module_json_path`, `resources_index_path`, `resource_table_path`, `compiled_resource_dir` | 当前 `layout.rs`; CLI path logic | hvigor `PathInfo.getIntermediatesRes` |
| `command.rs` | `RestoolCommand`, `RestoolConfig`, `RestoolCommandBuilder`, `RestoolConfigBuilder` | `build_command`, `build_config_json`, `add_module_resources`, `add_har_resources`, `add_ids` | 新增，从 current compiler command assembly | `restool-command-builder.js`, `restool-file-config-builder.js` |
| `compiler.rs` | `ResourceCompileConfig`, `ResourceCompileInput`, `ResourceCompileOutput`, `ResourceInfo`, `CompiledEntry`, `ResourceTypeCode`, `ResourceCompiler` | `ResourceCompiler::new`, `compile`, `copy_resources`, `write_index`, `write_table`, `emit_compiled_entries` | 当前 `compiler.rs` 保留，构造参数 plan 化 | `compile-resource.js`, `abstract-compile-resource.js` |
| `profile.rs` | `ResourceProfileInput`, `ResourceProfileWriter` | `write_restool_config`, `write_resource_profile`, `module_json_resource_refs` | 当前 `profile.rs` | `process-resource.js` |
| `table.rs` | `ResourceTable`, `ResourceTableEntry` | `write_resource_table`, `sort_entries`, `type_sort_order` | 当前 `write_resource_table` 保留 | `ResourceTable.txt` |
| `index.rs` | `ResourcesIndex`, `ResourcesIndexEntry` | `write_resources_index`, `index_entry_for_resource` | 当前 `write_resources_index` 保留 | `resources.index` |
| `ids.rs` | `ResourceIdAllocator`, `ResourceIdRange` | `next`, `allocate_type`, `allocate_entry` | 当前 `ResourceIdAllocator` 保留 | restool ID allocation |
| `media.rs` | `MediaCompressionConfig`, `CompressionFilter`, `CompressionRule` | `from_build_options`, `resolve_filter_paths`, `check_media_filter` | current media logic + new | `opt-compression-builder.js` |
| `element.rs` | `ResourceElementParser`, `ResourceEntry`, `EntryType` | `ResourceElementParser::parse_element_file()`, `EntryType::from_name()` | 当前 `element.rs` 保留 | element resource XML/JSON parse |
| `dependency_resources.rs` | `DependencyResourceInput`, `DependencyResourceMap` | `from_dependency_graph`, `ordered_har_resources`, `ordered_hsp_resources` | new, from CLI/resource dep logic | HAR/HSP resource dependencies |
| `compiled.rs` | `CompiledResourceProcessor`, `CompiledResourceInput`, `CompiledResourceOutput`, `ResourcePackageEntry`, `PreloadSystemSoJson` | `process_compiled_resources`, `copy_compiled`, `verify_outputs`, `package_entries`, `process_preload_system_so_json` | new | `process-compiled-resources.js::processPreloadSystemSoJson`, `PackageHapCA` dependency on compiled resources |
| `incremental.rs` | `ResourceIncrementalPlan`, `ResourceIncrementalInput`, `ResourceIncrementalDecision` | `from_cache_inputs`, `is_preview_incremental_enabled`, `changed_resources`, `restool_incremental_args` | new | `DefaultSdk.isPreviewCompileResourceIncrement`, preview/resource incremental branch |
| `startup_resource.rs` | `StartupResourceInput`, `StartupResourceValidator`, `StartupResourceOutput` | `from_profile_artifacts`, `validate_task_dependencies`, `write_startup_resource_config` | new | `CompileResource::processStartupConfig/checkStartupTaskDependencies/validateDependencies` |
