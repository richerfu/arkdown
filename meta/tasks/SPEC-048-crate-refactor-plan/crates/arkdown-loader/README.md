# crates/arkdown-loader

Action: Delete.

## 删除原因

`arkdown-loader` 保留 legacy `ArkdownPlugin`/`PluginDriver` 栈，与 SPEC-048 目标不兼容。生产插件管线必须只使用 rolldown plugin trait，插件由 `arkdown-cli` 组装，`arkdown` 接收 trait objects。

## 上游对应

| 上游逻辑 | 上游文件 | arkdown 迁入位置 |
| --- | --- | --- |
| rollup plugin lifecycle | `arkts-pack.js::getBuiltInPlugins`, `runHvigorCompile` | `arkdown` + rolldown APIs |
| custom compile plugin wrapper | `arkts-pack.js::loadCustomCompilePlugins`, wrapper hooks | `arkdown-cli::cmd::build::plugins` |
| transform/load/resolve hooks | hvigor-arkts-compose plugins | corresponding `arkdown-plugin-*` adapters |

## 当前代码迁移目标

```text
PluginDriver
  -> rolldown plugin pipeline in arkdown
parse/load/transform helpers
  -> owning plugin crate or arkdown-ets-transform
legacy tests
  -> plugin crate tests or bundler integration tests
```

## 删除步骤

1. `arkdown` 移除 `arkdown-loader` dependency。
2. CLI plugin construction 改为 `Vec<Box<dyn rolldown_plugin::Plugin>>` 或项目内等价 trait object。
3. legacy loader tests 迁到 active plugin/bundler tests。
4. 删除 `crates/arkdown-loader/` 和 workspace references。

## 退出条件

- `rg "arkdown-loader|ArkdownPlugin|PluginDriver" crates Cargo.toml` 无生产命中。
- `arkdown` 不依赖 concrete plugin crates，也不依赖 loader crate。

