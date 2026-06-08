# crates/arkdown-ohos-schema

Action: Add/Keep.

## 目标职责

`arkdown-ohos-schema` 拥有 raw OHOS JSON/JSON5 schema：`build-profile.json5`、`app.json5`、`module.json5`、`oh-package.json5`、loader json、pack.info、signing、profile JSON。它只定义 schema 和 serde/JSON5 parse shape，不做 build policy。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| project/module build profile | `src/options/build/*.js`, `src/options/configure/*.js` | `build_profile.rs` |
| app profile | `AppScope/app.json5`, `src/options/configure/app-*.js` | `app_json.rs` |
| module profile | `src/options/configure/module-*.js` | `module_json.rs` |
| OHPM package | `oh-package.json5`, `oh-package-lock.json5` | `oh_package.rs` |
| loader output | `generate-loader-json.js`, `abstract-generate-loader-json.js` | `loader_json.rs` |
| pack.info | `make-pack-info.js`, `abstract-make-pack-info.js` | `pack_info.rs` |
| signing config | `validate-sign.js`, `sign/command-builder-impl/*.js` | `signing.rs` |
| router/share/startup/privacy/forms 等 profile | `process-*.js`, `pac-process/*.js` | `profiles/*.rs` |

## 目标目录

```text
src/
  lib.rs
  build_profile.rs
  app_json.rs
  module_json.rs
  oh_package.rs
  loader_json.rs
  pack_info.rs
  signing.rs
  profiles/
    mod.rs
    router.rs
    share.rs
    startup.rs
    privacy.rs
    forms.rs
    shortcut.rs
    distro_filter.rs
    permission_reason.rs
```

## 文件作用和内部结构

- `build_profile.rs`: project/module build profile raw structs，字段保留 hvigor 命名，normalize 放 `arkdown-config`。
- `app_json.rs`: app/bundle raw schema。
- `module_json.rs`: module/abilities/extensionAbilities/pages/profile references raw schema。
- `oh_package.rs`: `OhPackageJson5`、lockfile raw entry、dependency spec raw value。
- `loader_json.rs`: `modulePathMap`、`hspNameOhmMap`、compile mode、project root path 等 loader schema。
- `pack_info.rs`: pack.info raw schema。
- `signing.rs`: signingConfigs、material、profile/cert/store file 字段。
- `profiles/*.rs`: router/share/startup/privacy/forms 等 profile 文件 schema。

## 串联关系

```text
arkdown-ohos-schema
  -> arkdown-config normalizes build/profile config
  -> arkdown-session stores parsed raw plus normalized snapshots
  -> arkdown-profile/resource/pack/sign consume normalized plans
```

## 迁移要点

- 不在 schema crate 内做路径 resolve、dependency classify、SDK probing。
- raw schema 字符串可以是 `Box<str>`；长生命周期共享字段由 session 转成 `Arc<str>`。
- JSON5 parse error 应带 schema file path diagnostic attachment，但不包含 CLI 输出策略。

