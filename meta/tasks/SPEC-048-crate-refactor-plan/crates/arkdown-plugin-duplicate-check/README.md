# crates/arkdown-plugin-duplicate-check

Action: Keep.

## 目标职责

`arkdown-plugin-duplicate-check` 是 output validation plugin，检测重复文件名或 hvigor parity 要求的冲突。它应只消费 rolldown output 和 context 中的 case policy，不做额外 IO。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| `checkDuplicatedFileNamesPlugin` | `plugins/check-duplicate-filenames-plugin.js` | plugin output validation |
| case sensitivity behavior | `use-sensitive-check-plugin.js` | resolver handles path case; duplicate plugin handles output name conflict |

## 目标目录

```text
src/
  lib.rs
  plugin.rs
  diagnostics.rs
```

## 文件作用和内部结构

- `plugin.rs`: generate/bundle end output validation。
- `diagnostics.rs`: duplicate diagnostics and output path attachments。
- `lib.rs`: plugin constructor export。

## 串联关系

```text
BundleOutput
  -> DuplicateCheckPlugin facts/diagnostics
  -> BuildRunner diagnostic aggregation
```

## 迁移要点

- 构造参数来自 `ModuleCompileContext` 或 output policy plan。
- 不做 source/resource/package 文件扫描。

