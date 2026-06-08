# crates/arkdown-profile

Action: Keep.

## 目标职责

`arkdown-profile` 只负责 profile/loader/pkgContext/module JSON 处理：merge/process profile、router/share/startup/privacy/forms、loader.json、pkgContextInfo、package-time module json。它消费 typed plans，不拥有 OHPM、dependency graph 构建、SDK probing 或 project discovery。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| MergeProfile | `src/tasks/merge-profile.js`, `abstract-merge-profile.js` | `merge_profile.rs`, `module_json_writer.rs` |
| ProcessProfile | `src/tasks/process-profile.js` | `process_profile.rs` |
| GenerateLoaderJson | `src/tasks/generate-loader-json.js`, `abstract-generate-loader-json.js` | `loader_json.rs` |
| GeneratePkgContextInfo | `src/tasks/generate-pkg-context-info.js` | `pkg_context_info.rs` |
| ProcessRouterMap | `src/tasks/process-router-map.js` | `router_map.rs` |
| ProcessShareConfig | `src/tasks/process-share-config.js` | `process_share_config.rs` |
| ProcessStartupConfig | `src/tasks/process-startup-config.js` | `process_startup_config.rs` |
| ProcessOhPackageJson for HAR | `src/tasks/har/process-oh-package-json.js` | `oh_package_writer.rs` |
| permission/privacy/forms/distro filters | `pac-process/*.js`, profile processors | existing `process_*` files |

## 目标目录

```text
src/
  lib.rs
  plan.rs
  writer.rs
  merge_profile.rs
  process_profile.rs
  module_json_writer.rs
  loader_json.rs
  pkg_context_info.rs
  router_map.rs
  process_share_config.rs
  process_startup_config.rs
  process_privacy_profile.rs
  process_permission_reason.rs
  process_shortcut_profile.rs
  process_distro_filter.rs
  form_profile.rs
  har_abilities.rs
  bytecode_har.rs
  oh_package_writer.rs
```

## 文件作用和内部结构

- `plan.rs`: `ProfileProcessPlan`、`LoaderJsonPlan`、`PkgContextInfoPlan`、`PackageModuleJsonPlan`。
- `writer.rs`: deterministic JSON writer，统一 spacing/order/hvigor parity。
- `merge_profile.rs`: app/module/target/product profile merge。
- `process_profile.rs`: 处理 merged module json，输出 `process_profile/default/module.json`。
- `module_json_writer.rs`: module json 写入 package/intermediates 的共用 writer。
- `loader_json.rs`: `modulePathMap`、`hspNameOhmMap`、projectRootPath、compileMode、runtimeOnly/bytecode 信息。
- `pkg_context_info.rs`: SO 默认 context、module/package context info，来自 session graph/native inventory。
- `router_map.rs`: router map 生成和 fingerprint input，替代 standalone router-map plugin。
- `process_share_config.rs`: share config profile writer。
- `process_startup_config.rs`: startup config writer，含 preload/system SO 相关 profile 引用。
- `process_privacy_profile.rs`: PAC/privacy profile 处理。
- `bytecode_har.rs`: ByteCode HAR metadata 写入，不拥有 OHPM。
- `oh_package_writer.rs`: HAR generated oh-package JSON writer。

## 串联关系

```text
ModuleCompileContext
  -> BuildPlanBuilder creates ProfileProcessPlan
  -> arkdown-profile outputs profile/loader/pkgContext artifacts
  -> arkdown-resource, arkdown, arkdown-pack consume artifacts
```

## 迁移要点

- 删除 `dependency_graph` 和 OHPM registry/cache 逻辑。
- profile processors 不再读 module/profile files，改用 `ProfileStore` 中的 parsed inputs。
- router map invalidation 并入 `ProfileStore` fingerprint 后删除 `arkdown-plugin-router-map`。

