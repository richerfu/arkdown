# crates/arkdown-resolver

Action: Keep.

## 目标职责

`arkdown-resolver` 负责纯 OHOS module resolution。它消费 `ResolverIndex`、package index、SDK module index、case check policy，返回 resolved module ID/path/OHM URL。它不得在 hot path 读取 package 文件、向上查找包根或按字符串包含判断 `oh_modules`。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| OHPM resolver plugin | `@ohos/hvigor-arkts-compose/dist/src/plugins/resolve/index.js` | `resolver.rs`, `oh_modules.rs` |
| `getResolveModules` | `arkts-pack.js::getResolveModules` | session-built `ResolverIndex` input |
| SDK api/global module path | `load-sdk-functions.js`, `sdkSupportExtensions` | `sdk.rs` |
| normalized OHM URL | `use-new-ohm-url-plugin.js` | `ohm_url.rs` |
| case sensitive check | `use-sensitive-check-plugin.js` | `case_check.rs` |
| importee fact collection | `collect-importers-plugin.js` | resolver output metadata consumed by plugin |

## 目标目录

```text
src/
  lib.rs
  error.rs
  resolver.rs
  index.rs
  specifier.rs
  oh_modules.rs
  ohm_url.rs
  sdk.rs
  case_check.rs
```

## 文件作用和内部结构

- `resolver.rs`: `OhosResolver`，输入 `ResolveRequest` 和 immutable `ResolverIndex`。
- `index.rs`: package roots、source roots、dependency aliases、extension set、SDK module table。
- `specifier.rs`: specifier 解析桥接 `arkdown-path::ModuleSpecifier`。
- `oh_modules.rs`: component-based OHPM path classification 和 package ownership 查询。
- `ohm_url.rs`: `OhmUrlBuilder`，按 bundle/module/package 信息生成 runtime URL。
- `sdk.rs`: SDK module path/index lookup。
- `case_check.rs`: directory-entry based case-exact check，替代 sensitive-check plugin。
- `error.rs`: unresolved import、case mismatch、ambiguous package、unsupported extension。

## 串联关系

```text
arkdown-session::DependencyGraphSnapshot
  -> arkdown-session::ResolverIndexBuilder
  -> arkdown-resolver::OhosResolver
  -> arkdown-plugin-ohos-resolve
  -> arkdown bundle graph
```

## 迁移要点

- 移除 hot path `fs::read`、`read_dir`、package json walk-up。
- resolver result 必须携带 package ownership fact，供 HAR dedup/module meta/importee 收集使用。
- case check 从独立 plugin 迁入 resolver 后，删除 `arkdown-plugin-sensitive-check`。

