# crates/arkdown

Action: Keep.

## 目标职责

`arkdown` 只负责 rolldown bundling 和 bundle output 转换。它接收 `BundlePlan` 和已经构造好的 rolldown plugin trait objects，不导入具体 `arkdown-plugin-*`、`arkdown-profile`、`arkdown-session`。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| `runArkPack` rollup config | `@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js::generateConfig` | `plan.rs`, `bundler.rs` |
| rollup build/write/generate | `@ohos/hvigor-arkts-base/dist/src/build.js` | `bundler.rs` |
| output config/source map path transform | `arkts-pack.js::getOutputConfig` | `output.rs` |
| entry input object | `arkts-pack.js::getCompileInputObject` | `compile_entry.rs` |
| plugin list ordering | `arkts-pack.js::getBuiltInPlugins` | plugin objects supplied by CLI, not built here |

## 目标目录

```text
src/
  lib.rs
  plan.rs
  bundler.rs
  compile_entry.rs
  output.rs
  facts.rs
  diagnostics.rs
```

## 文件作用和内部结构

- `plan.rs`: `BundlePlan`，包含 input entries、output dir、compile mode、sourcemap policy、cache policy、plugin list。
- `bundler.rs`: `Bundler`，封装 rolldown 调用，返回 `BundleOutput`。
- `compile_entry.rs`: entry object 到 rolldown input 的转换，不读 SDK，不读 module profile。
- `output.rs`: chunk/assets/source map output conversion，artifact path serialization 走 `arkdown-path`。
- `facts.rs`: bundle 后收集的 importee、module meta、HAR dedup、duplicate check 等 facts 的 typed result。
- `diagnostics.rs`: rollup/rolldown error 到 arkdown diagnostic 的转换。

## 串联关系

```text
arkdown-cli::BuildPlanBuilder
  -> BundlePlan
  -> arkdown::Bundler
  -> BundleOutput + PluginFacts
  -> obfuscate/ts2abc/pack stages
```

## 迁移要点

- 移除对 `arkdown-profile` 和所有 concrete plugin crates 的依赖。
- 不在 bundler 内部派生 project root、module root、loader path。
- 不在 bundler 内部决定 cache key，只消费 cache plan。

