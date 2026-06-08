# crates/arkdown-abc

Action: Keep.

## 目标职责

`arkdown-abc` 拥有 Rust ABC compiler 和 SPEC-049 已合并的 upstream-shaped AOT chain：source/ABC input 编译、Program cache、ProgramRegistry、dependency BFS、npm entries、global optimization gate、proto/cache/ABC output。SPEC-048 不拆分此 crate，但要求后续 typed plan 直接进入这条链，不能绕回旧的 ad hoc source-to-final-ABC facade。

本 crate 不读取 hvigor 项目文件、不生成 loader json、不决定 module artifact layout。它只消费上游已经归一化的 `AotCompileOptions`/后续 `AbcCompilePlan`，输出 ABC/proto/cache artifact。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| SDK genAbc plugin | `load-sdk-functions.js` 中 `genAbc` | `compiler.rs` facade |
| native es2abc AOT runner | `es2panda/aot/main.cpp::Run/ResolveAndGenerate/GenerateProgram` | `aot/runner.rs`, `aot/output.rs` |
| filelist/options parse | `es2panda/aot/options.cpp::CollectInput*` | `aot/options.rs` |
| compile queue | `compiler/core/compileQueue.cpp::Compiler::CompileFiles/CompileFileJob` | `aot/compile_queue.rs`, `aot/source_compiler.rs` |
| ABC input | `aot/options.cpp::CollectInputAbcFile`, `abc2program` path | `aot/abc_input.rs`, `abc2program.rs` |
| Program map/order | `std::map<std::string, ProgramCache*> programsInfo` | `aot/program_registry.rs`, `program/model.rs` |
| dependency pruning | `aot/resolveDepsRelation.cpp::DepsRelationResolver::Resolve` | `aot/deps.rs` |
| npm entries | `CompileNpmModuleEntryList` | `aot/npm_entries.rs` |
| Program cache/proto | `RetrieveProgramFromCacheFiles`, `EmitProgramCacheJob` | `aot/cache_store.rs`, `program_cache.rs`, `proto_panda.rs` |
| ArkTS/TS/JS AST lowering | es2panda parser/lowering equivalent | `ir/builder.rs`, `ir/*` |
| ABC binary emit | `emitFiles.cpp::EmitFileJob/EmitMergedAbcJob` | `program/to_abc.rs`, `emit/*` |
| source map/debug records | `BuildLineNumberProgram`, debug sections | `ir/debug_lnp.rs`, `emit/debug_info.rs` |

## 目标目录

```text
src/
  lib.rs
  compiler.rs
  error.rs
  names.rs
  isa.rs
  abc_reader.rs
  abc2program.rs
  cache.rs
  dependency.rs
  npm_entries.rs
  optimizer.rs
  program_cache.rs
  proto_panda.rs
  widget.rs
  aot/
    options.rs
    runner.rs
    compile_queue.rs
    source_compiler.rs
    abc_input.rs
    program_registry.rs
    deps.rs
    npm_entries.rs
    cache_store.rs
    optimizer.rs
    output.rs
    ohmurl.rs
  program/
    ids.rs
    model.rs
    to_abc.rs
    dump.rs
  ir/
  emit/
```

## 文件作用和内部结构

- `aot/options.rs`: `AotCompileOptions`、`SourceFile`、`CompileContextInfo`、`AotOutput`。后续增加 `AotOptionsParser`，承接当前 `parse_filesinfo` free function，并补齐 non-merge output column。
- `aot/runner.rs`: `AotRunner::run()`。这是 production ABC 入口，串联 compile queue、ABC input consistency、npm entries、dependency BFS、optimization、output。
- `aot/compile_queue.rs`: `CompileFileScheduler` 和 compile jobs，负责 source/ABC/cache 分流。
- `aot/program_registry.rs`: `ProgramRegistry`、`ProgramKey`。后续增加 `ProgramKeyCodec`，承接当前 `abc_input_program_key/is_abc_program_key/split_abc_program_key`。
- `aot/deps.rs`: `DepsRelationResolver`、`ResolvedDepsRelation`。后续把 `resolve_deps_relation` 改成 `DepsRelationResolver::resolve_relation`。
- `aot/output.rs`: `AotOutputGenerator`、`AotOutputResult`。后续增加 `AotOutputPathPolicy`、`AotBytesWriter`、`Base64Encoder`，补齐 non-merge output path 和 dump symbol table。
- `aot/optimizer.rs`: `ProgramOptimizer`。当前 global analysis call site 已接好，但 `AnalysisProgram` 仍是 stub，需要后续 CFG/liveness state。
- `program/model.rs`: `Program`、`ProgramMetadata`、`ProgramCacheEntry`，是 AOT chain 的中间表示。
- `program/to_abc.rs`: 当前 `program_to_abc_file/registry_to_abc_file` 是 free function；目标迁入 `ProgramToAbcConverter`。
- `program/dump.rs`: 当前 dump helpers 是 free function；目标迁入 `ProgramDumper`。
- `ir/*`: OXC AST 到 Program/record/function/literal/module metadata 的 lowering。
- `emit/*`: final ABC binary writer，输出顺序必须来自 `ProgramRegistry` key order 或 typed dense ID order。
- `abc_reader.rs`、`abc2program.rs`、`program_cache.rs`、`proto_panda.rs`: ABC/proto/cache 读写和 ABC input rebuild。

## 串联关系

```text
arkdown-ts2abc::Ts2abcPlan
  -> AotCompileOptions
  -> AotRunner::run()
     -> CompileFileScheduler::run_all()
        -> SourceCompiler / AbcInputProgramCompiler / ProgramCacheStore
        -> ProgramRegistry
     -> ProgramOptimizer
     -> NpmModuleEntryCompiler
     -> DepsRelationResolver
     -> AotOutputGenerator
  -> AotOutputResult
  -> arkdown-pack
```

## 迁移要点

- 不引入 `index_vec`。
- cache/path identity 由 plan 提供，不在 ABC 内部猜 module path。
- final binary table order 保持 sequential/deterministic。
- `compiler.rs` 的旧 facade 只作为迁移来源；production plan 迁到 `AotRunner` 后，应删除或收窄到内部 adapter，不能保留旧 source-to-final 兼容入口。
- 当前 public free function 必须全部降为 private helper 或迁到 owner 类型 associated method；不能作为最终 crate API。
- SPEC-049 已知缺口必须在 `files.md` 中保留 owner 和实现计划，直到 parity fixture 证明等价。
