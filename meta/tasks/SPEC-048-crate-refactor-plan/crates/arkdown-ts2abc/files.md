# File API Plan: crates/arkdown-ts2abc

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `args.rs` 是 CLI-style parser，应保留为 compatibility/test facade，但 production 用 `Ts2abcPlan`。
- `derived_output_path`、record/path derivation 迁到 plan builder，不在 arg parser 中做 stage policy。
- production plan 必须由 loader_json metadata、bundle JS outputs、source map policy、bytecode HAR metadata 共同生成，不能只从 command-line args 反推。
- 本 crate 必须覆盖 SDK fast-build `rollup-plugin-gen-abc.js`、module/bundle ABC、single-file emit、AOT 和 legacy/fallback ABC 路径；不能只实现一条 genAbc happy path。
- JS compat、lazy import、bytecode obfuscation、source map preserve/update 都必须由 typed plan 控制，不能从 CLI flags 或文件名临时推断。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `Ts2abcCompiler`, `Ts2abcPlan`, `Ts2abcOutput` | `Ts2abcCompiler::new(plan).compile()` | current exports narrowed | SDK `genAbc` typed facade |
| `error.rs` | `Ts2abcError`, `Ts2abcDiagnostic` | `Ts2abcError::invalid_plan`, `abc`, `io` | new | compile facade errors |
| `plan.rs` | `Ts2abcPlan`, `Ts2abcInput`, `Ts2abcOutput`, `CompileUnitPlan`, `RecordNamePlan`, `LoaderMetadataInput`, `BundleJsInput`, `ModuleSourceInput` | `Ts2abcPlan::validate`, `compile_units`, `abc_output_path`, `record_names`, `from_loader_and_bundle`, `from_transform_facts` | new from `args.rs::Ts2abcCommand` plus bundle output handoff | SDK `genAbc`, `arkts-pack.js::generateConfig`, `GenerateLoaderJson` |
| `module_source.rs` | `ModuleSourceInputKind`, `ModuleSourceFilePlan`, `ShouldTransformJsCompat`, `LazyImportPlan` | `collect_from_bundle`, `collect_from_transform_output`, `process_lazy_import`, `apply_js_api10_compat` | new | `ark_compiler/transform.js` |
| `compiler.rs` | `Ts2abcCompiler`, `Ts2abcCompileOutput`, `AbcCompileMode` | `Ts2abcCompiler::new`, `compile`, `compile_unit`, `compile_bundle`, `compile_module`, `to_abc_plan` | new, calls `arkdown-abc` | SDK `genAbc` plugin / `CompileArkTS` |
| `module_mode.rs` | `ModuleAbcGenerator`, `ModuleAbcPlan`, `ModuleAbcOutput`, `ModuleSourceList` | `ModuleAbcGenerator::new`, `generate_module_abc`, `collect_module_sources`, `write_module_cache`, `to_abc_jobs` | new from fast build genAbc | SDK `ark_compiler/generate_module_abc.js`, `rollup-plugin-gen-abc.js::generateBundle` module mode |
| `bundle_mode.rs` | `BundleAbcGenerator`, `BundleAbcPlan`, `BundleAbcOutput`, `BundleSourceList` | `BundleAbcGenerator::new`, `generate_bundle_abc`, `collect_bundle_sources`, `write_bundle_cache`, `to_abc_jobs` | new from current bundle handoff | SDK `ark_compiler/generate_bundle_abc.js`, `rollup-plugin-gen-abc.js::generateBundle` bundle mode |
| `bytecode_har.rs` | `BytecodeHarCompileInput`, `BytecodeHarCompileOutput` | `from_loader_metadata`, `blocked_dependencies`, `extra_compile_files` | new from profile bytecode metadata | ByteCode HAR optimization |
| `bytecode_obfuscation.rs` | `BytecodeObfuscationPlan`, `BytecodeObfuscator`, `BytecodeObfuscationOutput` | `BytecodeObfuscationPlan::from_obfuscation_plan`, `BytecodeObfuscator::run`, `rewrite_module_records`, `emit_name_cache` | new from obfuscate/ABC handoff | SDK `ark_compiler/bytecode_obfuscator.js`, `terser-plugin.js` handoff |
| `records.rs` | `RecordNameDeriver`, `RecordNameInput` | `derive_record_name`, `derive_package_record`, `derive_worker_record` | current `args.rs` record/path derivation | ABC record names |
| `single_file.rs` | `SingleFileEmitPlan`, `SingleFileDeclarationOutput` | `from_project_config`, `process_module_parsed`, `write_declarations` | new | `genAbc.moduleParsed`, `beforeBuildEnd` singleFileEmit |
| `aot.rs` | `AotCompilePlan`, `AotCompiler`, `AotOutput`, `AotSdkVersionGate` | `AotCompilePlan::from_loader_and_sdk`, `AotCompiler::new`, `compile_aot`, `check_fixed_sdk_version`, `emit_an_build_outputs` | new | SDK `lib/gen_aot.js`, `DefaultSdk.checkAotFixedSdkVersion`, SDK `main.js` AOT `anBuildOutPut/anBuildMode` |
| `legacy_bundle.rs` | `LegacyAbcPlan`, `LegacyAbcCompiler`, `LegacyGenAbcPluginMode`, `LegacyArkWorkerPlan`, `LegacyToolchainDelegate` | `LegacyAbcPlan::from_compat_request`, `LegacyAbcCompiler::compile`, `submit_worker_jobs`, `delegate_to_legacy_toolchain`, `emit_hvigor_equivalent_error` | current `arkdown-abc` compatibility + new fallback plan | legacy SDK `gen_abc_plugin.js`, worker `run-ark.js::submitArkCompileWork`, non-fast-build fallback |
| `npm_entries.rs` | `NpmEntryProtoWriter`, `NpmEntriesInfo`, `NpmEntryRecord` | `from_module_sources`, `write_entries_proto`, `record_dependency_entries` | new | SDK genAbc npm entries/proto handling |
| `preflight.rs` | `PreAbcValidation`, `JsImportArkTsCheck`, `SharedModuleCheck` | `check_js_importing_arkts`, `check_shared_modules`, `coverage_sources` | new | `check_import_module.js`, `check_shared_module.js` |
| `source_map.rs` | `AbcSourceMapPlan`, `SourceMapPackageInfo` | `preserve_source_map`, `update_source_map`, `merge_bundle_maps` | current output source map handling | SDK `generate_sourcemap.js`, `preserveSourceMap` |
| `cache.rs` | `AbcCachePlan`, `ModuleAbcCache`, `BundleAbcCache`, `UnsupportedChangedFileSet` | `from_cache_bridge`, `restore_module_cache`, `restore_bundle_cache`, `write_abc_cache`, `unsupported_changed_files` | new from current task cache and SDK cache | SDK `ark_compiler/cache.js`, `generateConfig.unsupportedChangedFiles` |
| `js_compat.rs` | `JsCompatTransform`, `JsCompatOptions` | `needs_babel_class_properties`, `transform_api10_js` | new | `compatibleSdkVersion <= 10` Babel branch |
| `args.rs` | `VersionQuery`, `Ts2abcCommand`, `ArgParser`, `InputSpec` | `ArgParser::parse_command()`, `ArgParser::split_flag_value()`, `ArgParser::parse_bool_flag()`; compatibility-only | current `args.rs`保留但不用于 production pipeline | ts2abc CLI compatibility |

## 与 ArkTS transform 的串联

`arkdown-ts2abc` 必须接收 `arkdown-ets-transform::TransformOutput`：

| Transform output | ts2abc 行为 |
| --- | --- |
| `Js` from `.ets/.ts` | 作为 JS module source，按 record name 进入 module/bundle ABC。 |
| `TsForAbc` from `.ets/.ts` | 保留为 TS/ETS module source 或 IR，按 `UseTsHar/byteCodeHar/processTs` 分支进入 ABC。 |
| `.js` passthrough | 处理 lazy import；`compatibleSdkVersion <= 10` 时执行 JS compat transform。 |
| `.json` module | 作为 JSON module source 进入 ABC。 |
| declaration-only | 不编 ABC，只进入 HAR declaration/package staging。 |

因此 production plan 不允许只从 CLI args 推导输入；必须消费 loader_json、bundle chunk graph、transform facts、source map policy、bytecode HAR metadata。
