# crates/arkdown-plugin-ets-transform

Action: Keep.

## 目标职责

`arkdown-plugin-ets-transform` 是薄 rolldown adapter。它负责接收 transform hook 的 id/code，查 context 中的 transform plan，调用 `arkdown-ets-transform`，把结果转换成 rolldown hook output。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| SDK `etsTransform` plugin hook | `load-sdk-functions.js` 加载 SDK plugins | adapter hook |
| transform cache invalidation | `invalid-cache-plugin.js` | 不在本 crate 决策，消费 cache result |
| ArkTS source map | `arkts-pack.js::getOutputConfig` | hook output source map |
| current crate transform implementation | `crates/arkdown-plugin-ets-transform/src/transform/*` | 迁到 `arkdown-ets-transform` |

## 目标目录

```text
src/
  lib.rs
  plugin.rs
  hook.rs
  options.rs
```

## 文件作用和内部结构

- `lib.rs`: export plugin constructor only。
- `plugin.rs`: `EtsTransformPlugin`，持有 `Arc<ModuleCompileContext>` 或 `Arc<EtsTransformPlan>`。
- `hook.rs`: rolldown transform hook 到 domain transformer 的桥接。
- `options.rs`: narrow adapter options，不重复定义 transform core option。

## 串联关系

```text
arkdown-cli::plugins
  -> EtsTransformPlugin::new(context or plan)
  -> rolldown transform hook
  -> arkdown-ets-transform
```

## 迁移要点

- 删除当前 crate 内 `transform/` 内核代码，保留 adapter。
- hook 内不得读 SDK/component/profile/package 文件。
- 插件不拥有 cache storage，只报告 transform facts。

