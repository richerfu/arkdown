# crates/arkdown-config

Action: Keep.

## 目标职责

`arkdown-config` 负责配置归一化和 SDK manifest probing：选择 product/target/build mode、解析 build options、选择 SDK 组件、形成 `NormalizedBuildConfig`。raw schema 迁到 `arkdown-ohos-schema`，启动发现和文件读取由 `arkdown-session` 触发。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| product/target 选择 | `src/common/find-target-product.js`, build option managers | `normalize.rs` |
| project/module build profile loader | `src/utils/loader/file/*build-profile-loader.js` | parse input from session, normalize here |
| SDK 版本/API 解析 | `src/sdk/sdk-util.js`, `default-sdk.js`, `ohos-sdk-loader.js`, `hmos-sdk-loader.js` | `sdk.rs` |
| strictMode/resOptions/arkOptions/signing | `src/options/build/*.js` | `build_options.rs` |
| runtimeOS/API model validation | `src/tasks/inspection/module-inspection.js` | diagnostics emitted during normalize |

## 目标目录

```text
src/
  lib.rs
  error.rs
  normalize.rs
  build_options.rs
  product.rs
  target.rs
  sdk.rs
  diagnostics.rs
```

## 文件作用和内部结构

- `normalize.rs`: `ConfigNormalizer`，输入 raw schema + selected CLI target，输出 `NormalizedBuildConfig`。
- `build_options.rs`: debuggable、compileMode、strictMode、resOptions、arkOptions、native/signing 等归一化类型。
- `product.rs`: product selection、bundleName/version 覆盖规则。
- `target.rs`: module target、runtimeOS、deviceTypes、build mode 归一化。
- `sdk.rs`: SDK component probing result、toolchain path metadata、API version parse。
- `diagnostics.rs`: 配置错误和 warning，保留 hvigor error file attachment。
- `error.rs`: typed config errors。

## 串联关系

```text
arkdown-session reads raw schema
  -> arkdown-config::ConfigNormalizer
  -> ProjectSnapshot.normalized_config
  -> ModuleCompileContext target config
```

## 迁移要点

- 不再直接扫描 project tree。
- 不拥有 OHPM package acquisition。
- 不生成 artifact path，artifact layout 归 `arkdown-session` + `arkdown-path`。
- SDK probing 只输出 snapshot 原料，不在后续 plugin hook 中重复 require SDK。

