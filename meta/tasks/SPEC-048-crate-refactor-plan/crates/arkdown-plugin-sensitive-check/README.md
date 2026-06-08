# crates/arkdown-plugin-sensitive-check

Action: Delete.

## 删除原因

case-sensitive path check 是 resolver/path policy，不应该作为独立 plugin 在 bundler hot path 做 IO。SPEC-048 要求 case-exact check 迁入 `arkdown-resolver::case_check`，路径行为由 `arkdown-path` 支撑。

## 上游对应

| 上游逻辑 | 上游文件 | arkdown 迁入位置 |
| --- | --- | --- |
| use sensitive check plugin | `plugins/use-sensitive-check-plugin.js` | `arkdown-resolver::case_check` |
| config caseSensitiveCheck | `ark-compile.js::initDefaultArkCompileConfig` | `arkdown-config` normalized option |
| path component/case behavior | filesystem reads in plugin | `arkdown-path` + resolver boundary |

## 当前代码迁移目标

```text
plugin hook case checking
  -> resolver case check during resolve
caseSensitiveCheck option
  -> NormalizedBuildConfig
diagnostics
  -> resolver diagnostic
```

## 删除步骤

1. `arkdown-config` 归一化 `strictMode.caseSensitiveCheck`。
2. `arkdown-resolver::case_check` 在 resolve boundary 做目录 entry 校验。
3. plugin resolver 输出 case mismatch diagnostic。
4. 移除 CLI/plugin dependency。
5. 删除 `crates/arkdown-plugin-sensitive-check/`。

## 退出条件

- case mismatch 测试在 resolver crate 覆盖。
- plugin hook 中无 case-check filesystem IO。
- `rg "arkdown-plugin-sensitive-check|sensitiveCheckPlugin" crates Cargo.toml` 无生产命中。

