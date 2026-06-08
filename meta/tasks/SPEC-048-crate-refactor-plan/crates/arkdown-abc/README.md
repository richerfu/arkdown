# crates/arkdown-abc

Action: Keep.

## 目标职责

`arkdown-abc` 拥有 Rust ABC compiler：IR、emit、reader、optimizer、program cache、字节码表。SPEC-048 不拆分此 crate，但要求 typed dense IDs、deterministic table layout、清理长字符串路径输入。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| SDK genAbc plugin | `load-sdk-functions.js` 中 `genAbc` | `compiler.rs` facade |
| Ark compile output to ABC | `worker/job.js -> runArkPack` | `compiler.rs`, `emit/*` |
| bytecode HAR optimization | `ark-compile.js`, HAR compile tasks | plan input from `arkdown-ts2abc` |
| source map and records | `arkts-pack.js` output | source map metadata input |

## 目标目录

```text
src/
  lib.rs
  compiler.rs
  error.rs
  ids.rs
  names.rs
  isa.rs
  optimizer.rs
  ir/
  emit/
  reader/
  tables/
```

## 文件作用和内部结构

- `compiler.rs`: `AbcCompiler`，输入 `AbcCompilePlan`，输出 `AbcArtifact`。
- `ids.rs`: ABC 内部 typed IDs，优先 `oxc_index::IndexVec`。
- `ir/*`: IR nodes、functions、literal arrays、records。
- `emit/*`: binary layout writer，最终顺序必须稳定。
- `reader/*`: ABC reader/tests。
- `optimizer.rs`: pass runner。
- `tables/*`: string/literal/method/class tables。

## 串联关系

```text
arkdown-ts2abc::Ts2abcPlan
  -> AbcCompilePlan
  -> arkdown-abc::AbcCompiler
  -> ABC artifact
  -> arkdown-pack
```

## 迁移要点

- 不引入 `index_vec`。
- cache/path identity 由 plan 提供，不在 ABC 内部猜 module path。
- final binary table order 保持 sequential/deterministic。

