# crates/arkdown-ts2abc

Action: Keep.

## 目标职责

`arkdown-ts2abc` 提供 upstream-compatible ts2abc/es2abc facade，把 Rolldown/OXC/ArkTS transform 产出的 JS/TS module source、bundle chunk、loader metadata、source map policy、bytecode HAR/AOT metadata 转成 ABC 编译计划。

当前代码只实现了 command-style facade：`args.rs` 解析 es2abc-like 参数，`driver.rs` 可选择 Rust `ArkdownAbcBackend` 或 SDK `SdkEs2abcBackend`。SPEC-048 的目标是新增 production `Ts2abcPlan` 和 `Ts2abcCompiler`，并把它串到 SPEC-049 的 `arkdown-abc::aot::AotRunner`。这些 facade 只作为迁移来源；如果没有明确对外 CLI contract，最终可以删除，不为内部 build pipeline 保留兼容壳。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| SDK genAbc plugin | `ark_compiler/lib/rollup-plugin-gen-abc.js::generateBundle`, `load-sdk-functions.js::genAbc` | `module_mode.rs`, `bundle_mode.rs`, `compiler.rs` |
| native es2abc wrapper | SDK `ts2abc.js`, `es2abc` CLI flags | current `args.rs`, `driver.rs`, target `compat_cli.rs` |
| ArkTS pack config | `arkts-pack.js::generateConfig/getCompileEnv/getPlugins` | `Ts2abcPlanBuilder` input from CLI BuildPlan |
| ArkUI/ArkTS transform output | SDK `etsTransform/apiTransform`, `ark_compiler/transform.js` | `module_source.rs`, `js_compat.rs` |
| module ABC | `generate_module_abc.js`, fast build genAbc | `module_mode.rs` |
| bundle ABC | `generate_bundle_abc.js`, non-fast-build bundle path | `bundle_mode.rs` |
| single file emit | `genAbc.moduleParsed`, `beforeBuildEnd` | `single_file.rs` |
| bytecode HAR | ByteCode HAR compile tasks/options | `bytecode_har.rs` |
| AOT | `gen_aot.js`, SDK `anBuildOutPut/anBuildMode` | `aot.rs` |
| legacy/fallback worker | `run-ark.js::submitArkCompileWork`, legacy `gen_abc_plugin.js` | `legacy_bundle.rs` |

## 目标目录

```text
src/
  lib.rs
  error.rs
  args.rs                 # migration source; optional external CLI parser only
  driver.rs               # migration source; split into backend/compiler/plan
  compile_context.rs
  version.rs
  plan.rs
  compiler.rs
  backend.rs
  file_info.rs
  module_source.rs
  module_mode.rs
  bundle_mode.rs
  bytecode_har.rs
  records.rs
  single_file.rs
  aot.rs
  legacy_bundle.rs
  npm_entries.rs
  source_map.rs
  cache.rs
  js_compat.rs
```

## 文件作用和内部结构

- `plan.rs`: `Ts2abcPlan`、`CompileUnitPlan`、`Ts2abcPlanBuilder`，只消费 session/build-plan 已归一化的 module facts，不读项目文件。
- `compiler.rs`: `Ts2abcCompiler`，选择 Rust AOT backend 或 SDK delegate，输出 `Ts2abcOutput`。
- `backend.rs`: `ArkdownAotBackend`、`SdkEs2abcBackend`、`BytecodeBackend`。当前 `driver.rs` 的 backend trait 和两个 backend 迁入此文件。
- `file_info.rs`: `FileInfoManifestWriter`、`FileInfoLine`。负责 upstream `filesInfo.txt` 列布局，尤其 non-merge output column。
- `module_source.rs`: 将 ArkUI/ArkTS transform output、JS passthrough、JSON module、declaration-only 分流成 ABC compile units。
- `module_mode.rs`/`bundle_mode.rs`: 对应 SDK genAbc module/bundle 模式，负责写 module/bundle source list、cache、ABC jobs。
- `aot.rs`: AOT plan 和 SDK fixed version gate；必须继续落到 `arkdown_abc::aot::AotRunner` 或 SDK AOT delegate。
- `args.rs`/`driver.rs`: 迁移来源；当前 public free function 必须迁到 `Ts2abcCompiler`/`BytecodeBackendRunner`，没有明确对外 CLI contract 时最终删除。

## 串联关系

```text
arkdown-ets-transform::TransformOutput
  + Rolldown BundleOutput
  + LoaderJson/PkgContext/SourceMap facts
  + ByteCode HAR/AOT/cache facts
  -> Ts2abcPlanBuilder
  -> Ts2abcPlan
  -> Ts2abcCompiler
     -> ArkdownAotBackend -> arkdown_abc::aot::AotRunner
     -> SdkEs2abcBackend -> SDK es2abc/ts2abc
  -> Ts2abcOutput
  -> pack entries
```

## 迁移要点

- 删除 CLI-style argument parsing ownership。
- path/record derivation 从 `ModuleCompileContext` 转成 plan。
- ABC 编译输入按 `CompileUnitId` 可并行，最终写入顺序稳定。
- ArkUI/ArkTS 不在本 crate 转换成 JS；转换属于 `arkdown-ets-transform`/Rolldown plugin。本 crate 只接收 transform facts 和 bundle outputs，并决定哪些 JS/TS/JSON/declaration 输入进入 ABC。
- OXC 迁移必须保留版本分支：`compatibleSdkVersion <= 10` 的 JS compat、`enableAnnotations` API gate、`useDefineSemantic`、bytecode HAR、AOT fixed SDK version、legacy fallback。
- `use_define_semantic` 是 plan 字段，SDK backend 原样传 `--use-define-semantic`；Rust backend 必须传入 `AotCompileOptions` 并由 `arkdown-abc` lowering 消费，不能默默丢弃。
- production plan 不允许从文件名临时猜 output path、record name、package name、source language；这些来自 loader metadata、bundle graph、module context 和 upstream-compatible fileInfo writer。
