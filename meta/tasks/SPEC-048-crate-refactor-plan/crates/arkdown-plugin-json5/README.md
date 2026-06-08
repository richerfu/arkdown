# crates/arkdown-plugin-json5

Action: Keep.

## 目标职责

`arkdown-plugin-json5` 是 JSON5 module load/emission adapter。它只处理被 rolldown 请求的 JSON5 module 内容，内容来自 hook input 或 source store，不在 hook 中重复读 project/profile/package 文件。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| rollup JSON plugin | `arkts-pack.js` 中 `@rollup/plugin-json` | JSON/JSON5 adapter |
| JSON5 project/profile parsing | hvigor `Json5Reader` usage | schema parse 已在 session/config/profile 完成 |
| module emission | current `module_emit.rs` | `module_emit.rs` |

## 目标目录

```text
src/
  lib.rs
  plugin.rs
  module_emit.rs
```

## 文件作用和内部结构

- `plugin.rs`: rolldown load/transform adapter。
- `module_emit.rs`: JSON5 value 到 JS module export 的 deterministic emission。
- `lib.rs`: narrow constructor export。

## 串联关系

```text
SourceStore or hook input
  -> Json5Plugin
  -> JS module output
```

## 迁移要点

- 不读 `module.json5` 或 profile JSON 作为 build stage input。
- 如果是 build metadata JSON，应由 `arkdown-profile`/schema/session 处理，不走 bundler plugin。

