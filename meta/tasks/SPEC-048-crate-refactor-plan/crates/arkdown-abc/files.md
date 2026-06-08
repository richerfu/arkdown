# File API Plan: crates/arkdown-abc

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 SPEC-049 AOT chain 是 production 基线：`AotCompileOptions -> AotRunner -> ProgramRegistry -> AotOutputGenerator`。后续 `AbcCompilePlan` 如果存在，只能是构造 `AotCompileOptions` 的 typed wrapper。
- `compiler.rs` 里的旧 `AbcCompiler` 只作为迁移来源；`arkdown-ts2abc` production path 必须迁到 `AotRunner`。迁完后旧 source-to-final `AbcFile` 入口删除或收窄为 crate-private adapter。
- 当前 public free function 必须全部进入 owner 类型 associated method；只允许 private helper 留在文件内部。
- binary output 顺序必须来自 `ProgramRegistry` lexicographic key order、`IndexVec` dense ID order 或显式 sorted order。
- ABC crate 不读项目配置、SDK 配置、loader json、oh-package、module json；这些输入必须由 `arkdown-ts2abc`/CLI plan 传入。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `AotRunner`, `AotCompileOptions`, `AotOutputResult` | 窄 re-export only | current exports narrowed；移除旧 `AbcCompiler` compatibility export、`program::dump`、`program::to_abc` free export | crate boundary；SDK `genAbc` facade |
| `error.rs` | `AbcError`, `AbcDiagnostic` | `AbcError::parse`, `lower`, `emit`, `read`, `unsupported` | current `error.rs` | es2abc diagnostics / compile failure |
| `compiler.rs` | `AbcCompiler`, `AbcCompileOptions`, `AbcModule`, `AbcInputFile`, `AbcProgramCacheInput`, `AbcArtifactResult`, `AbcCompilerMigrationAdapter` | `AbcCompilerMigrationAdapter::to_aot_options`, `compile_via_aot_runner`; current `AbcCompiler::new/compile_with_program_cache/compile_abc_inputs/compile_program_proto` are migration source only | current `compiler.rs`; `strip_typescript_types_pub`, `non_merge_metadata_records_pub` must become private adapter methods or move into `SourceCompiler` | SDK `load-sdk-functions.js::genAbc` input shape；final execution goes through `AotRunner` |
| `aot/options.rs` | `AotCompileOptions`, `SourceFile`, `SourceFileId`, `ScriptKind`, `ScriptExtension`, `AotOutput`, `CompileContextInfo`, `PkgInfo`, `FilesInfoError`, `AotOptionsParser`, `AotOutputFile` | `AotCompileOptions::effective_debug_info`, `global_script_kind`, `expected_program_count`; `SourceFile::new_source`, `new_abc_input`, `from_filesinfo_items`, `cache_key`; `ScriptKind::from_filesinfo_column`; `ScriptExtension::from_filename`; `CompileContextInfo::needs_dependency_relation`, `needs_package_version_update`; `AotOptionsParser::parse_filesinfo` | current structs；current `pub fn parse_filesinfo` migrates to `AotOptionsParser::parse_filesinfo`; add `SourceFile.output_file` for non-merge col 4 | `es2panda/aot/options.cpp::Options::Parse`, `CollectInputSourceFile`, `CollectInputAbcFile`, `CollectInputFilesFromFileList`, `NeedCollectDepsRelation` |
| `aot/runner.rs` | `AotRunner`, `AotRunPlan`, `AotRunState` | `AotRunner::new`, `run`; `AotRunState::compile_sources`, `check_merge_mode`, `compile_npm_entries`, `resolve_deps`, `generate_output` | current `AotRunner`; split internal steps only if needed | `es2panda/aot/main.cpp::Run`, `ResolveAndGenerate`, `GenerateProgram` |
| `aot/compile_queue.rs` | `CompileFileScheduler`, `CompileFileJob`, `CompileFileOutput`, `CompileFileCacheDecision`, `CompileFileJobFactory` | `CompileFileScheduler::new`, `run_all`; `CompileFileJob::run`, `load_or_compile`, `optimize_and_cache_program`; `CompileFileCacheDecision::from_options` | current `compile_queue.rs` | `compiler/core/compileQueue.cpp::Compiler::CompileFiles`, `CompileFileJob::Run`, `OptimizeAndCacheProgram` |
| `aot/source_compiler.rs` | `SourceCompiler`, `SourceCompileInput`, `SourceCompileOutput`, `TsSourceNormalizer` | `SourceCompiler::new`, `compile_source_file`, `compile_inline_source`; `TsSourceNormalizer::normalize` | current source path in `compiler.rs`/`compile_queue.rs`; `strip_typescript_types_pub` migrates here | es2panda source parser/lowering；OXC TS parse/codegen adapter |
| `aot/abc_input.rs` | `AbcInputProgramCompiler`, `AbcInputProgramSet`, `AbcInputMergeMode`, `AbcInputRecordPlan` | `AbcInputProgramCompiler::new`, `compile_file`, `split_records`, `rewrite_ohm_urls`, `insert_into_registry` | current `abc_input.rs`, `abc2program.rs` call sites | `CollectInputAbcFile`, ABC input rebuild, `CheckMergeModeConsistency` |
| `aot/program_registry.rs` | `ProgramRegistry`, `ProgramKey`, `ProgramId`, `ProgramKeyCodec` | `ProgramRegistry::new`, `add_expected`, `insert_source_program`, `insert_abc_program`, `insert_npm_program`, `get`, `get_mut`, `by_id`, `iter_in_key_order`, `drain_in_key_order`, `collect_in_key_order`, `check_merge_mode_consistency`, `validate_expected_count`, `retain_resolved`; `ProgramKeyCodec::abc_input_key`, `is_abc_input_key`, `split_abc_input_key` | current `ProgramRegistry`; current `abc_input_program_key/is_abc_program_key/split_abc_program_key` migrate to `ProgramKeyCodec` | upstream `programsInfo` `std::map` key semantics and merge consistency |
| `aot/cache_store.rs` | `ProgramCacheStore`, `ProgramCacheLookup`, `ProgramCacheWritePlan`, `PandaFileVersionPolicy` | `ProgramCacheStore::new`, `lookup_source`, `lookup_abc_input`, `write_source_program`, `write_abc_program`; `PandaFileVersionPolicy::for_sdk_cache`, `for_arkdown_cache` | current `cache_store.rs`; add pandaFileVersion policy | `RetrieveProgramFromCacheFiles`, `EmitProgramCacheJob`, cache hash/version |
| `aot/deps.rs` | `DepsRelationResolver`, `ResolvedDepsRelation`, `DynamicImportCollector`, `StaticImportCollector`, `DepsDumpWriter` | `DepsRelationResolver::new`, `resolve`, `resolve_relation`; `ResolvedDepsRelation::contains_program`, `retained_keys`, `dump`; `DynamicImportCollector::collect`; `StaticImportCollector::collect` | current `DepsRelationResolver`; current `pub fn resolve_deps_relation` migrates to `DepsRelationResolver::resolve_relation` | `aot/resolveDepsRelation.cpp::DepsRelationResolver::Resolve`, `VisitStaticImports`, `VisitDyanmicImports<true>` |
| `aot/npm_entries.rs` | `NpmModuleEntryCompiler`, `NpmModuleEntryProgram`, `NpmEntryListReader` | `NpmModuleEntryCompiler::new`, `compile`; `NpmEntryListReader::read`; `NpmModuleEntryProgram::from_entries` | current `NpmModuleEntryCompiler`; current `build_npm_program_from_content` migrates to `NpmModuleEntryProgram::from_entries` | `CompileNpmModuleEntryList` |
| `aot/optimizer.rs` | `ProgramOptimizer`, `GlobalAnalysisState`, `ProgramAnalysisPass`, `PostAnalysisOptimizationPass` | `ProgramOptimizer::new`, `from_compile_options`, `optimize_source_program`, `analyze_for_global_optimization`, `optimize_after_analysis`; `ProgramAnalysisPass::analyze`; `PostAnalysisOptimizationPass::optimize` | current `ProgramOptimizer`; `analyze_for_global_optimization` is wired stub and must gain state | `CompileFileJob::OptimizeAndCacheProgram`, `util::Helpers::AnalysisProgram`, `OptimizeProgram` |
| `aot/output.rs` | `AotOutputGenerator`, `AotOutputResult`, `AotOutputPathPolicy`, `AotBytesWriter`, `Base64Encoder`, `DumpSymbolTablePolicy` | `AotOutputGenerator::new`, `generate`, `emit_cache_jobs`, `emit_merged_abc`, `emit_single_abcs`, `emit_base64`, `emit_output_proto`; `AotOutputPathPolicy::path_for_source`; `AotBytesWriter::write_abc_file`; `Base64Encoder::encode`; `DumpSymbolTablePolicy::should_bypass_cache` | current `AotOutputGenerator`; current private helpers `derive_output_path/write_abc_file/base64_encode` migrate to types; add non-merge output column and dump symbol table policy | `aot/main.cpp::GenerateProgram`, `emitFiles.cpp::EmitFileQueue`, `EmitFileJob`, `EmitMergedAbcJob` |
| `aot/ohmurl.rs` | `OhmUrlRewriter`, `OhmUrlParser`, `PackageVersionUpdater`, `BundleNameUpdater`, `ExternalPackageSet` | `OhmUrlParser::pkg_name_from_import`, `pkg_name_from_ohmurl`, `record_name_from_ohmurl`; `OhmUrlRewriter::update_package_version`, `update_bundle_name`, `update_package_name`; `ExternalPackageSet::contains_ohmurl_pkg` | current public free functions migrate to these types | upstream normalized OHM URL helpers used by dependency BFS and ABC input package rewrite |
| `program/ids.rs` | `ProgramId`, `SourceFileId`, `RecordId`, `FunctionId`, `LiteralArrayId`, `StringId` | `new`, `index`, `from_usize` via `oxc_index::Idx` | current `program/ids.rs` | typed dense identity for Program/ABC tables |
| `program/model.rs` | `Program`, `ProgramMetadata`, `ProgramCacheEntry`, `SourceLang`, `ProgramRecordTable`, `ProgramLiteralArrayTable` | `Program::new_ecmascript`, `add_record`, table accessors; `ProgramCacheEntry::new_compiled`, `from_cache`, `mark_requires_post_analysis_opt` | current `program/model.rs` | upstream `pandasm::Program`, `ProgramCache` |
| `program/to_abc.rs` | `ProgramToAbcConverter`, `RegistryToAbcConverter`, `AbcConversionOptions` | `ProgramToAbcConverter::new`, `convert_program`; `RegistryToAbcConverter::new`, `convert_registry` | current `pub fn program_to_abc_file/registry_to_abc_file` migrate here | Program to final `AbcFile` before `EmitFileJob` |
| `program/dump.rs` | `ProgramDumper`, `ProgramDumpOptions`, `NormalizedProgramDumper` | `ProgramDumper::dump`, `format`, `format_normalized`; `NormalizedProgramDumper::format` | current `pub fn dump_program/format_program_dump/format_normalized_program_dump` migrate here | `--dump-program` / parity normalized dump |
| `abc_reader.rs` | `AbcReader`, `AbcFileRaw`, `AbcSectionReader` | `AbcReader::read_abc`, `read_sections`, `read_records` | current `abc_reader.rs` | ABC binary reader for ABC input/cache tests |
| `abc2program.rs` | `ProgramRebuilder`, `AbcProgramInput`, `AbcRecordRebuilder` | `ProgramRebuilder::from_abc_file`, `rebuild_programs`, `rebuild_record` | current `abc2program.rs` | ABC input to `Program` rebuild |
| `cache.rs` | `CacheFileMap`, `ProgramCacheFile`, `ProgramProtoFile`, `AbcProgramsCacheFile`, `CacheHashPolicy` | `CacheFileMap::read_manifest`, `get`; `ProgramCacheFile::read/write`; `ProgramProtoFile::read/write`; `CacheHashPolicy::source_hash`, `abc_input_hash` | current `cache.rs`; hash free functions move under `CacheHashPolicy` if exported | es2abc `--cache-file` manifest/protoBin |
| `dependency.rs` | `AbcDependencyContext`, `AbcDependency`, `DependencyResolver`, `DependencyCompactor` | `DependencyResolver::collect_dependencies`, `resolve_record_dependency`; `DependencyCompactor::compact_to_retained_programs` | current `dependency.rs`; migrate any still-needed behavior into `aot/deps.rs` then delete or crate-private this file | old `AbcCompiler` dependency pruning; final AOT path uses `aot/deps.rs` |
| `names.rs` | `PandasmNameCodec`, `PandasmRecordName`, `PandasmMethodName`, `LiteralArrayName` | `PandasmNameCodec::record_name`, `class_name_from_record`, `full_method_name`, `short_method_name`, `literal_array_key`, `literal_array_record_name` | current `names.rs` helpers migrate to associated methods if public | PandaASM naming |
| `isa.rs` | `InstructionSpec`, `IdKind`, `IdOperand`, `IsaTable`, internal YAML structs | `IsaTable::pandasm_spec`, `pandasm_opcode_for_bytecode`, `decode_pandasm_instruction`, `instruction_len`, `id_operands`, `encode_pandasm_instruction` | current `isa.rs` | ABC instruction set mapping |
| `optimizer.rs` | `BytecodeOptimizerOptions`, `BytecodeOptimizer`, `RecordOptimizer`, `MethodOptimizer` | `BytecodeOptimizerOptions::from_compile_options`; `BytecodeOptimizer::optimize_file`, `optimize_records`; `RecordOptimizer::optimize_record`; `MethodOptimizer::optimize_method` | current `optimizer.rs` free helpers migrate under optimizer types if exported | bytecode local optimizer |
| `program_cache.rs` | `ProgramCacheBuilder`, `ProgramProtoBuilder`, `RecordMetadata`, `FunctionMetadata`, `AnnotationProtoBuilder` | `ProgramCacheBuilder::build_program_cache`, `record_metadata`, `function_metadata`; `ProgramProtoBuilder::program_for_file`; `AnnotationProtoBuilder::append_annotation_records` | current `program_cache.rs`; `program_for_file/append_program_to_file` migrate to builder types if exported | Panda Program proto/cache generation |
| `proto_panda.rs` | proto structs, `PandaProtoCodec` | `PandaProtoCodec::encode_program`, `decode_program`, generated message accessors | current `proto_panda.rs` | Panda ABC proto model |
| `widget.rs` | `WidgetCompileConfig`, `WidgetCompiler`, `WidgetCompileInput`, `WidgetCompileOutput` | `WidgetCompiler::new`, `compile_widget`, `widget_record_names` | current `widget.rs` | hvigor widget compile path through ABC |
| `ir/builder.rs` | `ProgramBuilder`, `LoweringContext`, `BodyCompileResult`, `CodeGen`, `AnnotationRecordBuilder` | `ProgramBuilder::build_program`, `build_record`, `build_record_with_debug`, `compile_function_body`, `compile_class_elements`; `AnnotationRecordBuilder::annotation_records_from_source` | current `ir/builder.rs`; builder currently returns `Program` per SPEC-049 | OXC AST / es2panda lowering equivalent |
| `ir/lowering_context.rs` | `LoweringContext`, `CompileMode`, `LexicalScopeState` | context constructors/accessors | current file | lowering state |
| `ir/opcodes.rs` | `Insn`, `PandasmInsn`, `PandasmImm`, `JumpTargetInfo` | `Insn::opcode`, encode/map helpers; `JumpTargetInfo::mark_target`, `is_label` | current file; add jump target bit/table for dynamic import `IsLabel()` parity | bytecode instruction model, `Insn::IsLabel()` gap |
| `ir/function_records.rs` | `FunctionRecordBuilder`, `FunctionRecordLayout`, `FunctionRecordTable` | `FunctionRecordBuilder::new`, `build_function_record`, `build_method_record`, `finish_table` | current file | function record layout |
| `ir/function_lexical.rs` | `FunctionLexicalBuilder`, `LexicalVariableTable` | `collect`, `bind`, `finish` | current file | lexical env lowering |
| `ir/module_lexical.rs` | `ModuleLexicalBuilder`, `ModuleBindingTable` | `collect_imports_exports`, `finish` | current file | ESM module lexical lowering |
| `ir/module_record.rs` | `ModuleRecordBuilder`, `ModuleRequestTable`, `ModuleMetadataRecord` | `build_module_record`, `build_requests`, `build_commonjs_metadata` | current file | `moduleRecordIdx`, ESM/CJS metadata |
| `ir/record_layout.rs` | `RecordLayout`, `RecordLayoutBuilder` | `compute`, `fields`, `methods`, `literal_arrays` | current file | record layout |
| `ir/class_layout.rs` | `ClassLayout`, `ClassLayoutBuilder`, `PrivateFieldLayout` | `compute`, `private_fields`, `static_fields` | current file | class/private field lowering |
| `ir/debug_lnp.rs` | `DebugLineNumberProgram`, `DebugLnpBuilder` | `DebugLnpBuilder::new`, `emit_line`, `emit_column`, `finish` | current file | debug line number program |
| `ir/ast_analysis.rs` | `AstAnalysis`, `AstAnalysisPass` | `AstAnalysisPass::analyze_program`, `analyze_module`, `analyze_function` | current file | AST pre-analysis for lowering |
| `emit/file.rs` | `AbcFile`, `AbcFileEmitter`, `AbcSerializeOptions`, section emit state | `AbcFile::serialize_with_version`; `AbcFileEmitter::emit_file`, `write_sections`; `version_for_target_api` should move to `BytecodeVersionPolicy` if exported | current file | ABC binary writer / panda version |
| `emit/record.rs` | `ClassRecord`, `FieldRecord`, `MethodRecord`, `RecordEmitter` | `RecordEmitter::emit_record`, `emit_methods`, `emit_fields` | current emit record | record section writer |
| `emit/bytecode.rs` | `BytecodeEmitter`, `InstructionEncoder`, bytecode writer state | `BytecodeEmitter::emit_bytecode`; `InstructionEncoder::encode_instruction_stream` | current emit bytecode | bytecode section |
| `emit/debug_info.rs` | `DebugInfoEmitter`, `DebugInfoWriter` | `DebugInfoEmitter::emit_debug_info` | current file | debug section |
| `emit/strings.rs` | `StringTable`, `StringTableWriter`, `StringInterner` | `StringInterner::intern_string`; `StringTableWriter::emit_strings` | current file | ABC string table |

## 已删除或不应继续规划的旧文件

| 旧计划文件 | 当前状态 | 处理 |
| --- | --- | --- |
| `emit/index_section.rs` | 当前代码不存在 | 从计划删除；索引/section 写入由 `emit/file.rs` 和相关 emitter 承担 |
| `ir/assignment_targets.rs` | 当前代码不存在 | 从计划删除；assignment target 分析归 `ir/ast_analysis.rs`/builder 内部 |
| `ids.rs` crate root | 当前 typed IDs 在 `program/ids.rs` | 不再规划 root `ids.rs` |

## SPEC-049 未完全对齐项和实现计划

| 上游细节 | 当前状态 | 目标 owner/API | 实现要求 |
| --- | --- | --- | --- |
| non-merge filelist 第 4 列 output path | `parse_filesinfo` 测试说明了语义，但 `SourceFile` 未保存，`AotOutputGenerator` 仍从 key 派生 | `SourceFile.output_file`, `AotOutputPathPolicy::path_for_source` | `CollectInputSourceFile` 非 merge 模式读取 column 4；`emit_single_abcs` 必须按 source 对应 output 写出 |
| `dumpSymbolTable` cache bypass | 未建 flag | `AotCompileOptions.dump_symbol_table`, `DumpSymbolTablePolicy::should_bypass_cache` | cache lookup/emit 行为对齐 upstream dump symbol table 分支 |
| dynamic import `IsLabel()` | `Insn` 无 jump-target bit；当前只检查前一条 `LdaStr` | `JumpTargetInfo`, `DynamicImportCollector::collect` | instruction lowering 或 decode 时标记 label/jump target；收集时跳过 label DynamicImport |
| `AnalysisProgram` | `ProgramOptimizer::analyze_for_global_optimization` 是 wired stub | `GlobalAnalysisState`, `ProgramAnalysisPass::analyze`, `PostAnalysisOptimizationPass::optimize` | port bytecode optimizer CFG/liveness/inline profile 后再消费分析结果 |
| annotation records | 可生成但未做 ets2panda golden 覆盖 | `AnnotationRecordBuilder`, `AnnotationParitySuite` | API 20+ annotation fixture 对比 normalized Program/ABC dump |
| `pandaFileVersion` cache interop | arkdown cache 与 SDK cache 语义不完全互通 | `PandaFileVersionPolicy` | SDK cache 互操作时用 SDK version；arkdown-only cache 写 incompatible marker |
| `use_define_semantic` | option 字段存在但 lowering 语义未完整验明 | `SourceCompiler`, `ProgramBuilder` | OXC class field define semantic 必须在 lowering 中等价反映；SDK backend 继续原样传 flag |

## 串联图

```text
Ts2abcPlan
  -> AotCompileOptions
     -> source_files: IndexVec<SourceFileId, SourceFile>
     -> cache_files / compile_context_info / output / flags
  -> AotRunner::run()
     -> ProgramCacheStore::new()
     -> ProgramRegistry::new(source_count)
     -> CompileFileScheduler::run_all()
        -> SourceCompiler::compile_source_file()
        -> AbcInputProgramCompiler::compile_file()
        -> ProgramOptimizer::optimize_source_program() or analyze_for_global_optimization()
     -> ProgramRegistry::check_merge_mode_consistency()
     -> NpmModuleEntryCompiler::compile()
     -> DepsRelationResolver::resolve_relation()
     -> ProgramRegistry::retain_resolved()
     -> ProgramRegistry::validate_expected_count()
     -> AotOutputGenerator::generate()
        -> ProgramToAbcConverter / RegistryToAbcConverter
        -> ProgramCacheStore::write_source_program()
        -> AbcFile::serialize_with_version()
  -> AotOutputResult
```
