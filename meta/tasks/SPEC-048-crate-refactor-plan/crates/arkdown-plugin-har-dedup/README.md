# crates/arkdown-plugin-har-dedup

Action: Keep.

## 目标职责

`arkdown-plugin-har-dedup` 是 HAR dedup metadata adapter。它依据 session dependency ownership 判断模块是否来自 HAR/HSP/package，不再用路径 substring 判断 `oh_modules`。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| HAR deduplicate plugin/util | `plugins/har-deduplicate/*` | `plugin.rs` |
| dependency ownership | `src/project/dependency/dependency-manager.js` | session dependency graph input |
| package brief/source roots | `ark-compile.js::pkgNameToPkgBriefInfo` | package ownership fact |

## 目标目录

```text
src/
  lib.rs
  plugin.rs
  facts.rs
```

## 文件作用和内部结构

- `plugin.rs`: rolldown hook adapter，读取 resolver/module ownership fact。
- `facts.rs`: dedup candidates、retained modules、dependency package IDs。
- `lib.rs`: constructor export。

## 串联关系

```text
DependencyGraphSnapshot ownership
  -> HarDedupPlugin
  -> dedup facts
  -> bundle output conversion/package stage
```

## 迁移要点

- 删除 `contains("oh_modules")` 和 lossy string path 判断。
- hot path 不读 package json。
- dedup facts deterministic merge by package/module typed ID。

