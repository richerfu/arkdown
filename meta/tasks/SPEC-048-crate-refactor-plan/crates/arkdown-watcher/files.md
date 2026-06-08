# File API Plan: crates/arkdown-watcher

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `FileWatcher` 监听 raw directories；目标是监听 `BuildSession.input_inventory`。
- hot reload 事件保留，但 invalidation 来源必须是 fingerprint owner。
- watch 事件必须能映射到 hvigor `rollup-plugin-watch-change.js` 和 `arkts-pack.js::updateEventNames` 的 event name；不能只报告路径变化。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `FileWatcher`, `WatchPlan`, `WatchError` | `FileWatcher::new(plan).watch_session()` | current exports narrowed | hvigor watch/reload typed facade |
| `watcher.rs` | `SessionWatcher`, `WatchInput`, `WatchEvent` | `new`, `watch`, `poll`, `stop`, `events` | current `FileWatcher` | `arkts-pack.js::watch` |
| `invalidation.rs` | `InvalidationEvent`, `InvalidationReason`, `AffectedStage`, `AffectedModuleSet` | `from_file_event`, `match_fingerprint`, `affected_modules`, `explain` | new | `invalid-cache-plugin.js`, rollup watch cache |
| `event_name.rs` | `WatchEventName`, `WatchEventNameMapper`, `WatchChangedFileRecord` | `WatchEventNameMapper::from_session`, `event_name_for`, `changed_file_record`, `to_cache_event` | new | `rollup-plugin-watch-change.js`, `arkts-pack.js::updateEventNames/initUpdateEventNameHandler` |
| `debounce.rs` | `Debouncer`, `DebounceConfig`, `DebouncedBatch` | `push`, `flush_ready`, `force_flush` | new | watch delay |
| `hot_reload.rs` | `HotReloadEvent`, `HotReloadNotifier`, `HotReloadPlan` | `HotReloadEvent::from_invalidation`, `HotReloadNotifier::send`, `escape_json` | current `hot_reload.rs` | `hot-reload-ark-compile.js`, `cold-reload-ark-compile.js` |
| `deveco_ws.rs` | `DevEcoWorkspace`, `DevEcoWatchBridge` | `discover`, `notify`, `workspace_roots` | current `deveco_ws.rs` | DevEco integration |

## 必迁/必改

- watcher 不重新读 manifests/profile/package files。
- file event -> fingerprint -> affected stage 需要可诊断输出。
