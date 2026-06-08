# crates/arkdown-plugin-module-meta

Action: Keep.

## 目标职责

`arkdown-plugin-module-meta` 给 rolldown module 注入/收集 OHOS package metadata。它消费 session/resolver 的 package ownership，不再向上找 package json，不再用 mutex 维护反向 map。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| `moduleMetaInfoPlugin` | `plugins/module-meta-info-plugin.js` | `plugin.rs` |
| `findNodeEntryFiles` | same file and package info helpers | session source/package inventory |
| package brief info | `ark-compile.js::setPkgNameToPkgBriefInfo` | `pkg_brief_info.rs` |
| preload SO marker | project config `needPreloadSoFileSet` | plan fact input |

## 目标目录

```text
src/
  lib.rs
  plugin.rs
  pkg_brief_info.rs
  meta_compute.rs
  facts.rs
```

## 文件作用和内部结构

- `plugin.rs`: module parsed/transform side adapter。
- `pkg_brief_info.rs`: typed package brief info view from dependency graph。
- `meta_compute.rs`: pure metadata computation。
- `facts.rs`: module meta facts emitted by plugin。

## 串联关系

```text
DependencyGraphSnapshot + ResolverResult
  -> ModuleMetaPlugin
  -> module metadata facts
  -> loader/package metadata writers
```

## 迁移要点

- 移除 upward package walking。
- `pkgNameToPkgBriefInfo` 由 session graph 构造。
- 删除 `Arc<Mutex<...>>` collector，改为 typed fact output。

