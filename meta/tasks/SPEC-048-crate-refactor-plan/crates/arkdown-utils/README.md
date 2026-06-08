# crates/arkdown-utils

Action: Delete.

## 删除原因

`arkdown-utils` 是 generic helper bucket，违反 SPEC-048 的 ownership 规则。路径、hash、cache、IO helper 必须迁到 owning crate，不能继续作为跨 crate 杂物层存在。

## 上游对应

hvigor 也有大量 `utils/*`，但 arkdown 重构不能复制这种结构。上游 util 的逻辑应按归属拆分：

| 上游逻辑 | 上游文件 | arkdown 迁入位置 |
| --- | --- | --- |
| path/build dir helper | `src/utils/file-util.js`, path helpers | `arkdown-path` |
| JSON/read helper | `json-util.js`, loaders | `arkdown-session` startup 或 `arkdown-ohos-schema` parse |
| hash helper | `hash.js`, invalid cache helper | `arkdown-cache` or `arkdown-session::fingerprint` |
| package/dependency helper | `dependency-util.js`, `source-roots-utils.js` | `arkdown-session::dependency_graph` |
| task helper | `task-util.js` | `arkdown-cli::cmd::build` or domain plan |

## 当前代码迁移目标

```text
arkdown-utils path helpers
  -> arkdown-path
arkdown-utils hash/cache helpers
  -> arkdown-cache or arkdown-session::fingerprint
arkdown-utils filesystem helpers
  -> owning stage crate
```

## 删除步骤

1. 跑 `rg "arkdown-utils|arkdown_utils" crates Cargo.toml`。
2. 每个命中按 owning crate 迁移。
3. 最后生产调用方迁走后，从 root workspace members 和 dependencies 删除。
4. 删除 `crates/arkdown-utils/`。

## 退出条件

- `rg "arkdown-utils|arkdown_utils" Cargo.toml crates` 无生产命中。
- path helper 不再散落到 domain crates。
- `cargo metadata --format-version 1 --no-deps` 通过。

