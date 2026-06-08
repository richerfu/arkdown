# crates/arkdown-plugin-router-map

Action: Delete.

## 删除原因

router map 是 profile/session 输入，不应该作为独立 rolldown plugin 管理 cache invalidation。SPEC-048 要求 router map discovery、parse、fingerprint 归 `ProfileStore`，生成归 `arkdown-profile`。

## 上游对应

| 上游逻辑 | 上游文件 | arkdown 迁入位置 |
| --- | --- | --- |
| ProcessRouterMap task | `src/tasks/process-router-map.js` | `arkdown-profile::router_map` |
| ArkCompile router map config | `ark-compile.js::getRouterMapConfig` | `arkdown-session::ProfileStore`, dependency graph view |
| rollup router map plugin | `plugins/router-map-plugin.js` | delete, replace by session fingerprint and profile output |
| invalid cache router map hash | `invalid-cache-plugin.js::ROUTER_MAP_HASH` | `InputFingerprintSet` |

## 当前代码迁移目标

```text
router map file scanning
  -> arkdown-session::profile_store
router map generation
  -> arkdown-profile::router_map
router map cache invalidation
  -> arkdown-session::fingerprint
```

## 删除步骤

1. `ProfileStore` 记录 router map profile refs、parsed objects、fingerprints。
2. `arkdown-profile::router_map` 生成 hvigor parity output。
3. `InputFingerprintSet` 覆盖 router map content 和 dependency router map。
4. 移除 CLI/plugin dependency。
5. 删除 `crates/arkdown-plugin-router-map/`。

## 退出条件

- router map fixture 修改能触发 fingerprint invalidation。
- build output 中 router map artifact 仍与 hvigor parity 一致。
- `rg "arkdown-plugin-router-map|routerMapPlugin" crates Cargo.toml` 无生产命中。

