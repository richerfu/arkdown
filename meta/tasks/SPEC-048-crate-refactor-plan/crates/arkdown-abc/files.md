# File API Plan: crates/arkdown-abc

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- SPEC-048 不拆 crate，但要把 production facade plan 化，并继续保留现有 ABC reader/IR/emit/program_cache 逻辑。
- 对应的是当前 arkdown-abc 既有 ABC 编译实现，不是 hvigor JS。hvigor 侧只通过 SDK `genAbc`/ArkCompile 调用。
- dense table/order 必须 deterministic，内部 dense IDs 逐步迁到 `oxc_index::IndexVec`。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `AbcCompiler`, `AbcCompilePlan`, `AbcArtifact` | `AbcCompiler::new(plan).compile()` | current exports narrowed | SDK `genAbc`/ABC compiler typed facade |
| `compiler.rs` | `AbcCompiler`, `AbcCompilePlan`, `AbcCompileInput`, `AbcCompileOutput`, `AbcCompileOptions` | `AbcCompiler::new`, `compile`, `compile_source`, `compile_program`, `emit_file` | current `compiler.rs` | SDK genAbc equivalent |
| `error.rs` | `AbcError`, `AbcDiagnostic` | `AbcError::parse`, `lower`, `emit`, `read` | current `error.rs` | ABC compile diagnostics |
| `ids.rs` | `RecordId`, `FunctionId`, `LiteralArrayId`, `StringId`, `MethodId`, `ClassId` | `new`, `index`, `from_usize`; implement `Idx` | new from implicit indexes | dense table identity |
| `names.rs` | `PandasmNameCodec`, `PandasmRecordName`, `PandasmMethodName`, `LiteralArrayName` | `PandasmNameCodec::record_name`, `PandasmNameCodec::class_name_from_record`, `PandasmNameCodec::full_method_name`, `PandasmNameCodec::short_method_name`, `PandasmNameCodec::literal_array_key`, `PandasmNameCodec::literal_array_record_name` | current `names.rs` free helpers migrated to associated methods | ABC/PandaASM naming |
| `isa.rs` | `InstructionSpec`, `IdKind`, `IdOperand`, internal YAML structs | `pandasm_spec`, `pandasm_opcode_for_bytecode`, `decode_pandasm_instruction`, `instruction_len`, `id_operands`, `encode_pandasm_instruction` | current `isa.rs` | ABC instruction set |
| `optimizer.rs` | `BytecodeOptimizerOptions` | `optimize_file`, `optimize_records`, `optimize_method`, peephole/cfg/register frame helpers | current `optimizer.rs` | post-lowering optimizer |
| `abc_reader.rs` | `AbcReader`, `AbcFileRaw` | `read_abc`, `read_sections`, `read_records` | current `abc_reader.rs` | ABC binary reader |
| `abc2program.rs` | `ProgramRebuilder`, `AbcProgram` | `abc_to_program`, `record_to_program` | current `abc2program.rs` | ABC to internal program |
| `cache.rs` | `AbcProgramCache`, `CachedRecord` | `load`, `store`, `record`, `insert` | current `cache.rs` | ABC program cache |
| `dependency.rs` | `AbcDependency`, `DependencyRequest` | `collect_dependencies`, `resolve_record_dependency` | current `dependency.rs` | ABC record deps |
| `npm_entries.rs` | `NpmEntry`, `NpmEntryTable` | `collect_npm_entries`, `entry_for_package` | current `npm_entries.rs` | package entry metadata |
| `widget.rs` | `WidgetCompileInput`, `WidgetCompileOutput` | `compile_widget`, `widget_record_names` | current `widget.rs` | widget compile |
| `program_cache.rs` | `ProgramCacheBuilder`, `RecordMetadata`, many internal lowering cache structs | `build_program_cache`, `record_metadata`, `function_metadata`, annotation/proto helpers | current `program_cache.rs` | ABC proto/program cache |
| `proto_panda.rs` | generated/handwritten proto structs, `PandaProtoCodec` | `PandaProtoCodec::encode_program`, `PandaProtoCodec::decode_program`, generated message accessors | current `proto_panda.rs` | Panda ABC proto model |
| `ir/mod.rs` | `AbcFile`, `Record`, `FunctionRecord`, `LiteralArray`, module re-exports | constructors and IR accessors | current IR modules | ABC IR model |
| `ir/builder.rs` | `LoweringContext`, `BodyCompileResult`, `CodeGen`, many lowering helpers | `build_record`, `build_record_with_debug...`, `annotation_records_from_source`, `compile_function_body`, `compile_class_elements` | current `ir/builder.rs` | OXC AST -> ABC IR lowering |
| `ir/opcodes.rs` | `PandasmInsn`, `PandasmImm`, `Insn` | opcode encode/map helpers on `Insn` | current `opcodes.rs` | bytecode instruction model |
| `ir/lowering_context.rs` | `LoweringContext`, `CompileMode` specific state | context constructors/accessors | current file | lowering state |
| `ir/function_records.rs` | `FunctionRecordBuilder`, `FunctionRecordLayout`, `FunctionRecordTable` | `FunctionRecordBuilder::new`, `build_function_record`, `build_method_record`, `finish_table` | current file | function record layout |
| `ir/module_record.rs` | module record metadata | module record builders | current file | ESM/CJS module metadata |
| `ir/record_layout.rs` | record layout types | layout computation | current file | ABC record layout |
| `ir/class_layout.rs` | class/private field layout | layout computation | current file | class lowering |
| `ir/debug_lnp.rs` | debug line number program structs | line op builders | current file | debug info |
| `emit/file.rs` | `AbcFileEmitter`, section emit state | `emit_file`, `write_sections` | current emit file | ABC binary writer |
| `emit/record.rs` | record emit structs | `emit_record`, `emit_methods`, `emit_fields` | current emit record | record section writer |
| `emit/bytecode.rs` | bytecode writer state | `emit_bytecode`, `encode_instruction_stream` | current emit bytecode | bytecode section |
| `emit/debug_info.rs` | debug info writer | `emit_debug_info` | current file | debug section |
| `emit/index_section.rs` | index section writer | `emit_index_section` | current file | ABC index |
| `emit/strings.rs` | string table writer | `intern_string`, `emit_strings` | current file | ABC string table |

## 必改

- `compiler.rs` public entry must accept `AbcCompilePlan` from `arkdown-ts2abc` instead of ad hoc paths.
- Any table iteration affecting binary output must be sorted or dense-ID ordered.
