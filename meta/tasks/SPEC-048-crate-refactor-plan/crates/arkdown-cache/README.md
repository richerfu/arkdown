# crates/arkdown-cache

Action: Keep.

## 目标职责

`arkdown-cache` 拥有 cache storage、manifest、cache key materialization。它消费 `InputFingerprintSet` 和 stage cache plan，不决定 build 输入，不扫描项目，不读取 profile/package/SDK。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| rollup cache read/write | `@ohos/hvigor-arkts-base/dist/src/cache/rollup/*` | `store.rs`, `manifest.rs` |
| invalid cache plugin | `plugins/invalid-cache-plugin.js` | `key.rs` consumes fingerprint |
| Ark compile msgpack cache path | `MSGPACK_COMPILE_CACHE_DIR_NAME` usage | `namespace.rs` |
| obfuscation cache clean trigger | `ark-compile.js::checkAndCleanCache` | cache key includes obfuscation fingerprint |

## 目标目录

```text
src/
  lib.rs
  error.rs
  key.rs
  manifest.rs
  store.rs
  namespace.rs
  artifact.rs
```

## 文件作用和内部结构

- `key.rs`: `CacheKeyBuilder`，输入 session fingerprint + stage options。
- `manifest.rs`: cache manifest schema、version、entries、dependency wave identity。
- `store.rs`: cache read/write/commit API，atomic commit。
- `namespace.rs`: bundle/transform/abc/resource/pack cache namespace。
- `artifact.rs`: cache artifact metadata，不序列化 nondeterministic map order。
- `error.rs`: cache errors。

## 串联关系

```text
InputFingerprintSet
  -> BuildPlanBuilder creates CachePlan
  -> arkdown-cache
  -> stage cache hit/miss
```

## 迁移要点

- 移除 `arkdown-utils` hash helper 依赖。
- placeholder hash 禁止。
- cache key 必须解释来源：manifest/profile/source/SDK/resource/dependency graph/option。

