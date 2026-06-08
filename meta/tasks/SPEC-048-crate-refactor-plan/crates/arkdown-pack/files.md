# File API Plan: crates/arkdown-pack

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `Packager` 已含 HAP/HAR/HSP 打包逻辑和 JSON order helper；保留 archive writer，输入改成 `PackagePlan`。
- `PackInput`/`HarPackInput` 的路径和 module json 内容不得由 pack crate late-read project/module manifest。
- `ArchivePath` 和 zip entry slash handling 迁到 `arkdown-path`。
- HAR 打包不是简单 zip：必须覆盖 `ProcessHarArtifacts` 的 compiled source staging、bundle dependency resources、router/startup/share/native preload、obfuscation srcEntry、insight intent、declaration entry 处理。
- HAP/HSP/HAR package plan 必须保留上游 beforeAlwaysAction/precheck 行为，例如 HAR `expandImportPath` warning、bytecode HAR optimize dependency check、release sourcemap packing warning。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `Packager`, `PackagePlan`, `PackageOutput` | `Packager::new(plan).package()` | current exports narrowed | `PackageHap`/`PackageHsp`/`PackageHar` typed facade |
| `error.rs` | `PackError`, `PackDiagnostic` | `PackError::missing_input`, `archive`, `tool` | current `PackError` | package task errors |
| `plan.rs` | `PackagePlan`, `PackageKind`, `PackageEntry`, `PackageEntrySource`, `PackageOutput`, `PackageToolMode` | `PackagePlan::validate`, `ordered_entries`, `unsigned_output`, `pack_info_path` | new from current `PackInput`, `HarPackInput` | target task inputs |
| `packager.rs` | `Packager`, `PackInput`, `HarPackInput`, `PackAbility` | `Packager::new`, `package`, `package_hap`, `package_hsp`, `package_har`, `minify_package_intermediate_module_json` | current `packager.rs` 保留并拆 helpers | `package-hap.js`, `package-hsp.js`, `package-har.js` |
| `zip_writer.rs` | `ZipArchiveWriter`, `ZipEntryOptions` | `create_zip`, `add_file_deflate`, `add_file_stored`, `add_bytes_deflate`, `add_bytes_stored`, `finish` | current `zip_writer.rs` | package archive writing |
| `hap.rs` | `HapPackageBuilder`, `HapEntrySet` | `from_plan`, `add_profile`, `add_resources`, `add_assets`, `add_libs`, `write` | split from current `Packager::build_hap` logic | `PackageHap`, `pack-hap-request.js` |
| `hsp.rs` | `HspPackageBuilder`, `SharedHarOutput` | `from_plan`, `add_shared_resources`, `add_public_entries`, `write` | split from current HSP pack logic | `PackageHsp`, shared HAR/TGZ tasks |
| `shared_har.rs` | `SharedHarPlan`, `SharedHarResourceSet`, `SharedHarPackager` | `prepare_resources`, `copy_local_deps`, `copy_obfuscation_rules`, `package_shared_har` | new from HSP interface HAR staging | `prepare-shared-har-res.js`, `package-shared-har.js` |
| `shared_tgz.rs` | `SharedTgzPlan`, `SharedTgzEntry`, `SharedTgzPackager` | `from_signed_hsp_and_har`, `should_generate`, `package_tgz`, `cleanup_temp` | new | `package-shared-tgz.js` |
| `har.rs` | `HarPackageBuilder`, `HarEntrySet` | `from_plan`, `add_declarations`, `add_resources`, `add_oh_package`, `write` | current `HarPackInput` logic | `PackageHar`, `ProcessHarArtifacts` |
| `pack_info.rs` | `PackInfoWriter`, `PackInfoInput` | `from_plan`, `write`, `module_summary`, `package_summary` | new, uses schema | `make-pack-info.js` |
| `integrated_hsp.rs` | `IntegratedHspEntry`, `IntegratedHspPlan` | `IntegratedHspEntry::new`, `normalize_integrated_hsps`, `entries_from_graph` | current `integrated_hsp.rs` 保留 | `process-integrated-hsp.js` |
| `har_artifacts.rs` | `HarArtifactProcessor`, `HarArtifactOutput`, `HarArtifactStagingPlan`, `HarCompiledSourceStaging`, `HarBundleDependencyResource` | `process`, `copy_artifacts`, `copy_compiled_source_files`, `process_bundle_dependency_resources`, `copy_bundled_dependency_resources`, `collect_declarations`, `warn_release_sourcemaps` | new | `har/process-har-artifacts.js::copyCompiledSourceFileToTempDir/processBundleDepRes/copyBundledDepRes/processDeclarationEntry/warnWhenPackSourceMapsInReleaseMode` |
| `har_router_startup.rs` | `HarRouterMapPackager`, `HarStartupConfigPackager`, `HarShareConfigPackager`, `HarStartupTaskSrcEntry` | `process_har_router_map`, `process_har_startup_config`, `process_har_share_config`, `rewrite_startup_src_entry`, `check_app_preload_hint_src_entry` | new | `ProcessHarArtifacts::processHarRouterMap/processHarResStartupConfig/processShareConfig`, `PackageHar::checkAppPreloadHintStartupTasksSrcEntry` |
| `har_obfuscation.rs` | `HarObfuscationArtifactPlan`, `HarAbilitySrcEntryRewrite`, `ObfuscatedHarAbility`, `ObfuscationNameCache` | `get_obfuscation_src_entry_path`, `process_obfuscated_har_ability`, `get_obfuscated_har_ability_src_entry`, `load_obfuscation_name_cache` | new from current obfuscation/package handoff gaps | `ProcessHarArtifacts::getObfuscationSrcEntryPath/processObfuscatedHarAbility/getObfuscatedHarAbilitySrcEntry/getObfNameCacheObj` |
| `har_insight_intent.rs` | `HarInsightIntentPackager`, `HarInsightIntentSrcEntry`, `HarInsightIntentOutput` | `process_insight_intent`, `rewrite_insight_intent_src_entry`, `collect_bytecode_har_insight_entries` | new | `ProcessHarArtifacts::processInsightIntent/collectByteCodeHarInsightIntentSrcEntry` |
| `native_libs.rs` | `NativeLibPackEntry`, `NativeLibPackPlan` | `entries_from_inventory`, `validate_abi`, `ordered_entries` | new from current pack/resource/native handling | `ProcessLibs`, package tasks |
| `native_preload.rs` | `PreloadSystemSoPlan`, `PreloadSystemSoJsonWriter`, `BaseNativeLibsInput` | `from_transform_facts_and_graph`, `add_preload_system_so_json`, `write_base_native_libs_json`, `package_native_preload_entries` | new | `ProcessHarArtifacts::addPreloadSystemSoJson`, SDK `module_preload_file_utils.js`, `PackageHar` baseNativeLibsJson checks |
| `packing_check.rs` | `PackingCheckPlan`, `PackingCheckKind`, `PackingCheckDiagnostic` | `check`, `check_hap_hsp`, `check_har`, `validate_package_inputs` | new | `packing-check/packing-check.js`, `packing-check-har.js` |
| `package_precheck.rs` | `PackagePrecheck`, `HarPrecheck`, `ByteCodeHarOptimizeCheck`, `ExpandImportPathCheck`, `LocalDependencyCheck` | `PackagePrecheck::from_plan`, `check_har_expand_import_path`, `check_bytecode_har_optimize_dependencies`, `check_dev_global_local_dependencies`, `emit_warnings` | new | `PackageHar::doExpandImportPathCheck/beforeAlwaysAction`, bytecode HAR optimize dev/global local dependency checks |
| `debug_symbol.rs` | `DebugSymbolPlan`, `DebugSymbolOutput`, `NativeDebugSymbolMetadata` | `collect_debug_symbols`, `native_debug_symbol_metadata`, `report_outputs` | new | `collect-debug-symbol.js`, HAR `nativeDebugSymbol` metadata |
| `tool_options.rs` | `PackingToolOptions`, `PackingMode` | `for_hap`, `for_hsp`, `for_har`, `to_command_args` | from hvigor builder, current command assembly | `packing-tool-options.js` |

## 必迁/必改

- `ArchivePath` private helper 删除，改用 `ArtifactPathSerializer::zip_entry`。
- JSON field order helper 可以保留在 pack/profile writer，但不能读取 module json path 自己决定输入。
- `PackagePlan` 必须能表示 `GeneratePkgModuleJson -> ProcessCompiledResources -> PackageHap` 的输入边，以及 HSP `PrepareSharedHarRes -> PackageSharedHar -> PackageSharedTgz` 的旁路产物。
