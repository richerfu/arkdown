# crates/arkdown-ts2abc

Action: Keep.

## 目标职责

`arkdown-ts2abc` 提供 upstream-compatible ts2abc facade，把 bundle/ArkTS output 转成 `arkdown-abc` 可执行的 `AbcCompilePlan`。它不做 CLI-style 参数拼接，不自己派生 artifact path。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| CompileArkTS/es2abc facade | `ark-compile.js`, `worker/run-ark.js`, SDK genAbc | `plan.rs`, `compiler.rs` |
| compile mode/build mode/env | `arkts-pack.js::getCompileEnv` | `Ts2abcPlan` input |
| bytecode HAR | HAR compile tasks, ByteCode HAR options | `bytecode_har.rs` |
| record/module name derivation | ArkTS compile config | plan from session/profile |

## 目标目录

```text
src/
  lib.rs
  error.rs
  plan.rs
  compiler.rs
  bytecode_har.rs
  records.rs
```

## 文件作用和内部结构

- `plan.rs`: `Ts2abcPlan`，bundle entries、output ABC paths、record names、build mode、source map paths。
- `compiler.rs`: `Ts2abcCompiler`，调用 `arkdown-abc`。
- `bytecode_har.rs`: ByteCode HAR compile metadata。
- `records.rs`: deterministic record name derivation。
- `error.rs`: compile facade errors。

## 串联关系

```text
BundleOutput
  -> Ts2abcPlan
  -> arkdown-ts2abc
  -> arkdown-abc
  -> pack entries
```

## 迁移要点

- 删除 CLI-style argument parsing ownership。
- path/record derivation 从 `ModuleCompileContext` 转成 plan。
- ABC 编译输入按 `CompileUnitId` 可并行，最终写入顺序稳定。

