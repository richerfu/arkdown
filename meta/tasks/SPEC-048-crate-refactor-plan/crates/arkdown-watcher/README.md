# crates/arkdown-watcher

Action: Keep.

## 目标职责

`arkdown-watcher` 负责 watch/debounce/invalidation。它监听 `BuildSession` 的 input inventory，而不是递归扫描任意目录或重新执行全 CLI 逻辑。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| `runArkPack` watch mode | `arkts-pack.js::watch`, `getWatchConfig` | `watch.rs` |
| rollup watch cache | `hvigor-arkts-base/dist/src/build-steps/watch.js` | `invalidation.rs` |
| hot/cold reload tasks | `hot-reload-ark-compile.js`, `cold-reload-ark-compile.js` | `hot_reload.rs` |
| DevEco workspace integration | current `deveco_ws.rs` | `deveco_ws.rs` |

## 目标目录

```text
src/
  lib.rs
  watcher.rs
  invalidation.rs
  debounce.rs
  hot_reload.rs
  deveco_ws.rs
```

## 文件作用和内部结构

- `watcher.rs`: watcher lifecycle over session inventory。
- `invalidation.rs`: file event -> fingerprint owner -> affected module/stage。
- `debounce.rs`: debounce and batching。
- `hot_reload.rs`: hot/cold reload-specific invalidation policy。
- `deveco_ws.rs`: DevEco workspace bridge。

## 串联关系

```text
BuildSession.input_inventory
  -> arkdown-watcher
  -> invalidation result
  -> arkdown-cli BuildRunner partial rebuild
```

## 迁移要点

- 不维护另一套项目发现。
- watch 事件必须能追溯到具体 fingerprint。
- 不在 watcher 中写 artifact，写 artifact 归 runner/stage。

