# File API Plan: crates/arkdown-ts2abc

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 当前状态

当前 crate 已存在文件：

```text
src/
  lib.rs
  args.rs
  compile_context.rs
  driver.rs
  error.rs
  version.rs
```

这些文件实现的是 `ts2abc`/`es2abc` command facade 和 backend bridge，不是 SPEC-048 目标 production typed plan。目标文件 `plan.rs`、`compiler.rs`、`module_mode.rs`、`bundle_mode.rs`、`aot.rs`、`legacy_bundle.rs` 等尚未实现，必须在 Phase 3 前补齐。旧 facade 和 bridge 只作为迁移来源；若没有明确对外 CLI contract，迁完最后调用方后删除。

当前违反最终 API 规则的导出：

| 当前导出 | 迁移目标 | 原因 |
| --- | --- | --- |
| `driver.rs::compile_to_abc` | `Ts2abcCompiler::compile_options` 或 `BytecodeBackendRunner::compile_with_default_backend` | public free function，且隐藏 backend selection |
| `driver.rs::compile_to_abc_with_backend` | `BytecodeBackendRunner::compile_with_backend` | public free function |
| `version.rs::es2abc_target_bc_version` | `BytecodeVersionPolicy::for_es2abc` | public free function |
| `version.rs::ts2abc_script_target_bc_version` | `BytecodeVersionPolicy::for_ts2abc_script` | public free function |

## 迁移总则

- `args.rs` 只作为迁移来源或明确对外 CLI parser。production 不从 CLI flags 反推出 module graph、record name、package name、source map、cache path；无外部 contract 时删除。
- `driver.rs` 拆成 `backend.rs`、`compiler.rs`、`file_info.rs`。backend trait 可以存在，但编译入口必须挂在 `Ts2abcCompiler` 或 `BytecodeBackendRunner` 上；旧 bridge 文件迁完删除。
- Rust backend 必须生成 `arkdown-abc::aot::AotCompileOptions` 并调用 `AotRunner`。旧 `AbcCompiler::compile_with_program_cache` 只能作为过渡兼容。
- SDK backend 必须保留上游 `ts2abc.js`/`es2abc` 参数语义，包括 `--use-define-semantic`、`--enable-abc-input`、`--outputProto`、`--npm-module-entry-list`、`--remove-redundant-file`。
- ArkUI/ArkTS 到 TS/JS 的转换不属于本 crate；本 crate 的输入是 `arkdown-ets-transform` 和 Rolldown 输出后的 JS/TS/JSON/declaration facts。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `Ts2abcCompiler`, `Ts2abcPlan`, `Ts2abcOutput` | 窄 re-export only | current exports narrowed；移除 free function re-export；`Ts2abcCommand` 仅在明确外部 CLI contract 时 re-export | crate boundary；SDK genAbc facade |
| `error.rs` | `Ts2abcError`, `Ts2abcDiagnostic`, `Ts2abcUnsupportedFeature` | `Ts2abcError::invalid_plan`, `abc`, `io`, `sdk_failed`, `unsupported_feature` | current `error.rs` | ts2abc/es2abc diagnostics |
| `version.rs` | `BytecodeVersion`, `BytecodeVersionPolicy`, `TargetApiVersionPolicy` | `BytecodeVersion::new`, `Display`; `BytecodeVersionPolicy::for_es2abc`, `for_ts2abc_script`; `TargetApiVersionPolicy::default_target_api_version` | current `BytecodeVersion`; current free functions migrate to policy type | SDK `ts2abc.js --target-bc-version`, native es2abc `GetVersionByApi`; default API 24 |
| `compile_context.rs` | `CompileContextInfo`, `PkgInfo`, `CompileContextReader`, `CompileContextConverter` | `CompileContextReader::read_upstream_lenient`; `CompileContextInfo::needs_dependency_relation`, `needs_package_version_update`; `CompileContextConverter::to_aot_context` | current `CompileContextInfo::read_upstream_lenient`; current private `parse_update_version_info` stays private | upstream `compileContextInfo` JSON; `NeedCollectDepsRelation`; package version update |
| `args.rs` | `VersionQuery`, `Ts2abcCommand`, `ArgParser`, `InputSpec`, `Ts2abcCliParser` | `Ts2abcCommand::parse`; `Ts2abcCliParser::parse_command`; `ArgParser::parse`, `parse_one`, `parse_flag_value`, `compiler_output`, `apply_cache_file_mapping` | current `args.rs`; delete if no external CLI contract remains | SDK `ts2abc.js`/native es2abc command surface |
| `driver.rs` | `Ts2abcOptions`, `Ts2abcInput`, `Ts2abcOutput`, `Ts2abcDriverMigrationAdapter` | `Ts2abcOptions::compile`; `Ts2abcDriverMigrationAdapter::compile_options`, `compile_with_backend`, `default_record_name` | current `driver.rs`; current `compile_to_abc*` free functions migrate here, then old file deleted | bridge from old options to final `Ts2abcPlan`/backend |
| `backend.rs` | `BytecodeBackend` trait, `ArkdownAotBackend`, `SdkEs2abcBackend`, `BytecodeBackendRunner`, `SdkCommandBuilder` | `BytecodeBackend::compile`; `ArkdownAotBackend::compile`; `SdkEs2abcBackend::new`, `with_input_file_list`, `compile`; `BytecodeBackendRunner::compile_with_backend`; `SdkCommandBuilder::build_command`, `input_file_arg` | split from current `driver.rs::BytecodeBackend`, `ArkdownAbcBackend`, `SdkEs2abcBackend`, private command helpers | native SDK `es2abc` process invocation and Rust backend delegation |
| `plan.rs` | `Ts2abcPlan`, `Ts2abcPlanBuilder`, `CompileUnitPlan`, `CompileUnitId`, `Ts2abcOutputPlan`, `RecordNamePlan`, `LoaderMetadataInput`, `BundleJsInput`, `ModuleSourceInput`, `BytecodeBackendKind`, `Ts2abcParitySnapshot` | `Ts2abcPlanBuilder::new`, `from_module_compile_context`, `from_loader_and_bundle`, `from_transform_facts`, `build`; `Ts2abcPlan::validate`, `compile_units`, `output_plan`, `backend_kind`, `to_parity_snapshot`; `CompileUnitPlan::to_aot_source_file` | new from final CLI typed plan and current `Ts2abcOptions`; current CLI handoff is deleted after data fields move here | `arkts-pack.js::generateConfig/getCompileEnv`, `GenerateLoaderJson`, SDK genAbc input contract |
| `compiler.rs` | `Ts2abcCompiler`, `Ts2abcCompileOutput`, `Ts2abcCompileMode`, `AotOptionsBuilder` | `Ts2abcCompiler::new`, `compile`, `compile_unit`, `compile_module`, `compile_bundle`, `compile_aot`, `to_aot_options`; `AotOptionsBuilder::from_plan`, `build` | new; current `Ts2abcOptions::compile` and `ArkdownAbcBackend::compile` migrate here | SDK `load-sdk-functions.js::genAbc`, `rollup-plugin-gen-abc.js::generateBundle`, SPEC-049 `AotRunner` bridge |
| `file_info.rs` | `FileInfoManifestWriter`, `FileInfoLine`, `FileInfoColumnPolicy`, `CacheFileManifestWriter` | `FileInfoLine::from_compile_unit`, `to_upstream_line`; `FileInfoManifestWriter::write`; `FileInfoColumnPolicy::merge_columns`, `non_merge_columns`; `CacheFileManifestWriter::write` | current `driver.rs::file_info_line`, `generated_file_info_path`, `cache_file_arg` | `es2panda/aot/options.cpp::CollectInputFilesFromFileList`; SDK generated `filesInfo.txt` |
| `module_source.rs` | `ModuleSourceCollector`, `ModuleSourceInputKind`, `ModuleSourceFilePlan`, `TransformFactInput`, `JsonModulePlan`, `DeclarationOnlyPlan`, `LazyImportPlan` | `ModuleSourceCollector::collect_from_bundle`, `collect_from_transform_output`, `classify_source`, `process_lazy_import`, `skip_declaration_only` | new | SDK `ark_compiler/transform.js`, loader json source classification |
| `module_mode.rs` | `ModuleAbcGenerator`, `ModuleAbcPlan`, `ModuleAbcOutput`, `ModuleSourceList`, `ModuleCacheBridge` | `ModuleAbcGenerator::new`, `generate_module_abc`, `collect_module_sources`, `write_module_cache`, `to_aot_jobs` | new from fast build genAbc | SDK `generate_module_abc.js`, `rollup-plugin-gen-abc.js::generateBundle` module mode |
| `bundle_mode.rs` | `BundleAbcGenerator`, `BundleAbcPlan`, `BundleAbcOutput`, `BundleSourceList`, `BundleChunkMap` | `BundleAbcGenerator::new`, `generate_bundle_abc`, `collect_bundle_sources`, `write_bundle_cache`, `to_aot_jobs` | new from final `Ts2abcPlan` bundle facts; current CLI bundle handoff is deleted after migration | SDK `generate_bundle_abc.js`, non-fast-build bundle ABC |
| `bytecode_har.rs` | `BytecodeHarCompileInput`, `BytecodeHarCompileOutput`, `BytecodeHarDependencyFilter`, `BytecodeHarExtraFilePlanner` | `BytecodeHarCompileInput::from_loader_metadata`; `BytecodeHarDependencyFilter::blocked_dependencies`; `BytecodeHarExtraFilePlanner::extra_compile_files` | new from profile bytecode metadata | ByteCode HAR optimization tasks/options |
| `bytecode_obfuscation.rs` | `BytecodeObfuscationPlan`, `BytecodeObfuscator`, `BytecodeObfuscationOutput`, `ModuleRecordRewritePlan` | `BytecodeObfuscationPlan::from_obfuscation_plan`; `BytecodeObfuscator::run`, `rewrite_module_records`, `emit_name_cache` | new from typed obfuscation output and ABC plan facts; delete any temporary bridge file after migration | SDK `ark_compiler/bytecode_obfuscator.js`, terser/obfuscation ABC artifact flow |
| `records.rs` | `RecordNameDeriver`, `RecordNameInput`, `PackageRecordName`, `WorkerRecordName` | `RecordNameDeriver::derive_record_name`, `derive_package_record`, `derive_worker_record`, `derive_from_loader_metadata` | current `driver.rs::default_record_name`; CLI record/path derivation | ABC record name derivation from OHM URL/module context |
| `single_file.rs` | `SingleFileEmitPlan`, `SingleFileEmitter`, `SingleFileDeclarationOutput`, `ModuleParsedHookState` | `SingleFileEmitPlan::from_project_config`; `SingleFileEmitter::process_module_parsed`, `write_declarations`, `before_build_end` | new | SDK `genAbc.moduleParsed`, `beforeBuildEnd`, singleFileEmit |
| `aot.rs` | `AotCompilePlan`, `AotCompiler`, `AotOutput`, `AotSdkVersionGate`, `AnBuildOutputPlan` | `AotCompilePlan::from_loader_and_sdk`; `AotCompiler::new`, `compile_aot`, `emit_an_build_outputs`; `AotSdkVersionGate::check_fixed_sdk_version` | new | SDK `lib/gen_aot.js`, `DefaultSdk.checkAotFixedSdkVersion`, SDK `anBuildOutPut/anBuildMode` |
| `legacy_bundle.rs` | `LegacyAbcPlan`, `LegacyAbcCompiler`, `LegacyGenAbcPluginMode`, `LegacyArkWorkerPlan`, `LegacyToolchainDelegate` | `LegacyAbcPlan::from_compat_request`; `LegacyAbcCompiler::compile`, `submit_worker_jobs`, `delegate_to_legacy_toolchain`, `emit_hvigor_equivalent_error` | current compatibility path plus new fallback plan | legacy SDK `gen_abc_plugin.js`, worker `run-ark.js::submitArkCompileWork` |
| `npm_entries.rs` | `NpmEntryProtoWriter`, `NpmEntriesInfo`, `NpmEntryRecord`, `NpmEntryListPlan` | `NpmEntryProtoWriter::write_entries_proto`; `NpmEntriesInfo::from_module_sources`; `NpmEntryListPlan::record_dependency_entries` | new; AOT final compile handled by `arkdown-abc::aot::NpmModuleEntryCompiler` | SDK genAbc npm entries/proto handling |
| `preflight.rs` | `PreAbcValidation`, `JsImportArkTsCheck`, `SharedModuleCheck`, `AbcCoverageSourceCheck` | `PreAbcValidation::run`; `JsImportArkTsCheck::check_js_importing_arkts`; `SharedModuleCheck::check_shared_modules`; `AbcCoverageSourceCheck::coverage_sources` | new | SDK `check_import_module.js`, `check_shared_module.js` |
| `source_map.rs` | `AbcSourceMapPlan`, `SourceMapPackageInfo`, `SourceMapPreserver`, `SourceMapUpdater` | `AbcSourceMapPlan::from_bundle_output`; `SourceMapPreserver::preserve_source_map`; `SourceMapUpdater::update_source_map`, `merge_bundle_maps` | current CLI source map handling | SDK `generate_sourcemap.js`, `preserveSourceMap` |
| `cache.rs` | `AbcCachePlan`, `ModuleAbcCache`, `BundleAbcCache`, `UnsupportedChangedFileSet`, `Ts2abcCacheBridge` | `AbcCachePlan::from_cache_bridge`; `ModuleAbcCache::restore_module_cache`; `BundleAbcCache::restore_bundle_cache`; `Ts2abcCacheBridge::write_abc_cache`; `UnsupportedChangedFileSet::from_rollup_cache` | new from current `task_cache.rs`/`compile_arkts_cache.rs` | SDK `ark_compiler/cache.js`, `generateConfig.unsupportedChangedFiles` |
| `js_compat.rs` | `JsCompatTransform`, `JsCompatOptions`, `CompatibleSdkVersionPolicy` | `JsCompatTransform::transform_api10_js`; `JsCompatOptions::from_plan`; `CompatibleSdkVersionPolicy::needs_babel_class_properties` | new | `compatibleSdkVersion <= 10` Babel/class-properties branch |

## ArkUI/ArkTS 到 ABC 的分流合同

| 输入 | 由谁产生 | ts2abc 行为 | 上游对应 |
| --- | --- | --- | --- |
| `.ets/.ts` 经过 ArkTS transform 的 JS | `arkdown-ets-transform` / `arkdown-plugin-ets-transform` | `ModuleSourceCollector` 作为 JS/TS source compile unit；record/sourceFile/pkgName 来自 loader metadata | SDK `etsTransform/apiTransform` 后进入 genAbc |
| `.ets/.ts` 保留 TS/ETS source | transform facts 标记 `TsForAbc`、bytecode HAR/processTs 分支 | `CompileUnitPlan` 保留 `origin_source_lang` 和 source extension，交给 `AotOptionsBuilder` | SDK `processTs`, bytecode HAR |
| `.js` passthrough | Rolldown load/transform | `compatibleSdkVersion <= 10` 时经 `JsCompatTransform`；否则直接进入 compile unit | SDK JS compat Babel branch |
| `.json` module | JSON loader/plugin | `JsonModulePlan` 生成 JSON module compile unit或交给 bundle output | SDK JSON module handling |
| `.d.ts/.d.ets` declaration-only | transform/source inventory | 不进 ABC；进入 HAR declaration/package staging | SDK declaration emit |
| lazy import / dynamic import | Rolldown chunk graph + transform facts | `LazyImportPlan` 写入 module source/deps facts；最终 dependency BFS 由 `arkdown-abc::aot::DepsRelationResolver` 执行 | SDK lazy import handling + es2abc dependency relation |

## 与 SPEC-049 AOT 的串联图

```text
Ts2abcPlan
  -> Ts2abcCompiler::compile()
     -> PreAbcValidation::run()
     -> ModuleAbcGenerator / BundleAbcGenerator / AotCompiler / LegacyAbcCompiler
     -> AotOptionsBuilder::from_plan()
        -> AotCompileOptions
           -> source_files: SourceFile[]
           -> output/cache_files/compile_context_info/npm_module_entry_list
           -> use_define_semantic/enable_annotations/debug/opt flags
     -> ArkdownAotBackend::compile()
        -> arkdown_abc::aot::AotRunner::run()
     -> Ts2abcOutput
```

## 未实现或未完全对齐项

| 缺口 | 当前状态 | 实现计划 |
| --- | --- | --- |
| production `Ts2abcPlan` | 未实现；CLI 仍直接构造 `Ts2abcOptions`/handoff | 新增 `plan.rs`，由 `BuildPlanBuilder` 输入 loader/bundle/transform/cache facts，禁止读取项目文件；迁完后删除 handoff 文件 |
| Rust backend 仍走旧 `AbcCompiler` 多条路径 | `ArkdownAbcBackend` 调 `compile_with_program_cache/compile_abc_inputs` | `ArkdownAotBackend` 改成构造 `AotCompileOptions` 并统一调用 `AotRunner` |
| fileInfo non-merge output column | `Ts2abcInput.output` 有字段，SDK fileInfo writer 未作为独立 owner | `FileInfoLine::to_upstream_line` 对齐 merge/non-merge column；同时写入 `AotCompileOptions::SourceFile.output_file` |
| `use_define_semantic` Rust backend | SDK backend 传 flag；Rust backend未完整消费 | `AotOptionsBuilder` 保留字段，`arkdown-abc::SourceCompiler/ProgramBuilder` 实现 class field define semantic |
| module/bundle/single-file/AOT/legacy 分支 | 文档已有目标，代码未拆 | 分别实现 `module_mode.rs`、`bundle_mode.rs`、`single_file.rs`、`aot.rs`、`legacy_bundle.rs`，并写 parity fixture |
| public free function | `compile_to_abc*`、version free helpers仍 re-export | 迁到 `Ts2abcDriverMigrationAdapter`、`BytecodeBackendRunner`、`BytecodeVersionPolicy`，`lib.rs` 停止 re-export；迁完最后调用方后删除旧函数和旧 bridge 文件 |
