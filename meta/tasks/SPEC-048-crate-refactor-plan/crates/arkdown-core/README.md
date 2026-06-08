# crates/arkdown-core

Action: Keep.

## 目标职责

`arkdown-core` 必须只保留 domain-neutral foundation 类型：错误、dense id、module/package/chunk/resource/profile/compile unit ID、诊断、artifact metadata、source map schema。它不能拥有路径策略、raw OHOS schema、resolver policy、plugin API、build stage policy。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| task/module/project 的稳定标识 | `src/tasks/service/*.d.ts`, `src/project/data/*` | `ids.rs`, `module.rs`, `graph.rs` |
| compile event 和 artifact metadata | `@ohos/hvigor-arkts-compose/dist/src/analyzer/*.js` | `artifact.rs`, `diagnostic.rs` |
| source map record 形状 | `arkts-pack.js::getOutputConfig`, release map 处理 | `source_maps.rs` |

## 目标目录

```text
src/
  lib.rs
  error.rs
  ids.rs
  module.rs
  graph.rs
  artifact.rs
  diagnostic.rs
  source_maps.rs
```

## 文件作用和内部结构

- `lib.rs`: 只 re-export 稳定 foundation API。禁止 wildcard export implementation module。
- `error.rs`: `ArkdownError`、`Result<T>`、低层 typed error enum。CLI 可转成 `anyhow`，library crate 不反向依赖 CLI。
- `ids.rs`: `ModuleId`、`PackageId`、`DependencyNodeId`、`SourceFileId`、`ResourceFileId`、`ProfileFileId`、`CompileUnitId`、`ChunkId`、`PackEntryId`。
- `module.rs`: `ModuleKind`、`ArtifactKind`、`BuildMode`、`CompileMode`、`TargetName` 等纯值类型。
- `graph.rs`: `DependencyEdgeKind`、`DependencyScope`、基础 topo 结果类型。真实 graph 构建在 `arkdown-session`。
- `artifact.rs`: artifact metadata、logical output kind、stage output descriptors。真实路径类型在 `arkdown-path`。
- `diagnostic.rs`: severity、code、span/path attachment 的 domain-neutral 表达。
- `source_maps.rs`: source map section、mapping artifact metadata，供 bundler/abc/pack 共用。

## 串联关系

```text
arkdown-core
  -> 被所有 foundation/domain/plugin/CLI crate 依赖
```

`arkdown-core` 不依赖任何 arkdown workspace crate。新增类型如果需要 path/schema/session 信息，应放到 owning crate，而不是放进 core。

## 迁移要点

- 删除旧 plugin/context API。
- 删除字符串路径 helper。
- dense collection 使用 `oxc_index::IndexVec` 所需 typed ID。
- 不放 serde raw schema，schema 归 `arkdown-ohos-schema`。

