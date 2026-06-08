# crates/arkdown-plugin-ohos-resolve

Action: Keep.

## 目标职责

`arkdown-plugin-ohos-resolve` 是 `arkdown-resolver` 的 rolldown adapter。它消费 session-built resolver index，不做 package discovery，不向上查找 `oh-package.json5`。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| custom OHPM node resolve | `plugins/resolve/index.js` | `plugin.rs` 调用 `arkdown-resolver` |
| use native resolver fallback | `arkts-pack.js::checkRustResolver` | build plan option |
| collect importers | `collect-importers-plugin.js` | `importee.rs` typed fact |
| case check | `use-sensitive-check-plugin.js` | resolver result diagnostics |

## 目标目录

```text
src/
  lib.rs
  plugin.rs
  importee.rs
  diagnostics.rs
```

## 文件作用和内部结构

- `plugin.rs`: rolldown `resolve_id` hook adapter。
- `importee.rs`: direct/non-entry/compiled importee fact collector，keyed by module IDs。
- `diagnostics.rs`: resolver diagnostic 到 plugin diagnostic 的转换。

## 串联关系

```text
ModuleCompileContext.resolver_index
  -> arkdown-resolver::OhosResolver
  -> OhosResolvePlugin
  -> BundleOutput importee facts
```

## 迁移要点

- 删除本 crate 内路径解析和 package root 扫描。
- 不再使用字符串 `contains("oh_modules")`。
- importee facts 在 bundle 完成后 deterministic merge。

