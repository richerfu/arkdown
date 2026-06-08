# File API Plan: crates/arkdown-cache

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `lib.rs` 中 hash/cache/env/key/store 全在一个文件，需要拆分。
- `fnv1a_hash` 可作为内部实现，但 production cache key 由 `InputFingerprintSet` 驱动。
- 对应上游 `invalid-cache-plugin.js` 和 rollup cache service。
- cache 必须覆盖 plugin `shouldInvalidCache` hook、Rollup cache manifest、watch event name 和 compileEnv key；不能只做文件 hash store。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `BuildCache`, `BuildCacheOpener`, `CacheKeyBuilder` | `BuildCacheOpener::new(root).open()`, `CacheKeyBuilder::new().build()` | 当前 `lib.rs` 收窄 | hvigor rollup/task cache typed facade |
| `error.rs` | `CacheError`, `CacheDiagnostic` | `CacheError::io`, `decode`, `stale` | 当前 `CacheError` | cache service errors |
| `key.rs` | `CompileEnv`, `BundleCacheKey`, `CacheKey`, `StageCacheKey`, `CacheKeyMaterial` | `CompileEnv::from_plan`, `BundleCacheKey::from_fingerprints`, `CacheKey::stable_hash`, `compute_content_hash` | 当前 `CompileEnv`, `BundleCacheKey`, `CacheKey`, `compute_content_hash` | `invalid-cache-plugin.js`, `arkts-pack.js::getCompileEnv` |
| `manifest.rs` | `CacheManifest`, `CacheManifestEntry`, `CacheManifestVersion` | `load`, `store`, `entry`, `upsert`, `remove_stale` | current `CachedEntry` split | rollup cache manifest |
| `store.rs` | `BuildCache`, `CacheStore`, `CacheHit`, `CacheCommit` | `BuildCache::open`, `get`, `put`, `remove`, `commit`, `clear_namespace` | current `BuildCache` | `rollup-cache-service.js` |
| `namespace.rs` | `CacheNamespace`, `StageCacheNamespace` | `for_bundle`, `for_transform`, `for_abc`, `path_component` | new | hvigor task cache dirs |
| `artifact.rs` | `CachedArtifact`, `CachedArtifactKind`, `CachedArtifactFingerprint` | `from_stage_output`, `verify`, `restore` | current `CachedEntry` expansion | cache artifacts |
| `plugin_cache.rs` | `PluginCacheDecision`, `ShouldInvalidCacheBridge`, `PluginInvalidationInput`, `PluginInvalidationOutput` | `ShouldInvalidCacheBridge::new`, `ask_plugin`, `merge_decisions`, `force_invalidate`, `explain` | new | `arkts-pack.js::wrapperShouldInvalidCache`, `invalid-cache-plugin.js` |
| `rollup_cache.rs` | `RollupCacheStore`, `RollupCacheEntry`, `RollupCacheReadResult`, `RollupCacheWritePlan` | `RollupCacheStore::open`, `read_cache_with_event`, `write_cache`, `remove_cache`, `cache_directory` | current task cache + new bridge | `rollup-cache-service.js`, `build.js::readCacheWithEvent` |
| `event_name.rs` | `CacheEventName`, `UpdateEventNameHandler`, `ChangedFileEvent` | `CacheEventName::from_compile_env`, `UpdateEventNameHandler::update`, `event_for_changed_file`, `serialize_event_name` | new | `arkts-pack.js::updateEventNames/initUpdateEventNameHandler`, watch change plugin |
| `compile_env.rs` | `CompileEnvCacheMaterial`, `CompileEnvFingerprint`, `CompileEnvCachePolicy` | `from_bundle_compile_env`, `stable_material`, `fingerprint`, `affects_cache` | split from `key.rs` if it grows | `arkts-pack.js::getCompileEnv`, `generateConfig.compileEnv`, `useSourceHash/usePathPlaceholder` |

## 必迁/必改

- `CacheKey` input 不得直接从 partial env 字符串拼出来；必须包含 `InputFingerprintSet` stage slice。
- cache manifest serialization 必须 sorted。
- plugin invalidation 必须通过 `PluginCacheDecision` 合并进 stage cache key；watch 模式必须能解释 changed file 对应的上游 event name。
