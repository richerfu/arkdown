# crates/arkdown-session

Action: Add.

## 目标职责

`arkdown-session` 是 SPEC-048 的核心。它拥有启动发现、一次性读取、snapshot、dependency graph、profile store、SDK snapshot、resource/source inventory、artifact layout、fingerprint，以及 `ModuleCompileContext`。它不能依赖 profile/resource/pack/sign/cache/plugin crate。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| plugin factory 初始化全局数据 | `src/plugin/factory/plugin-factory.js` | `builder.rs`, `project.rs` |
| task service target data | `src/tasks/service/target-task-service.d.ts`, `module-task-service` | `module.rs`, `context.rs` |
| task initializer 注册阶段 | `src/utils/task-initializer.js` | plan 原料，不直接注册 task |
| dependency manager | `src/project/dependency/dependency-manager.js`, `module-dependency-info.js` | `dependency_graph.rs` |
| GeneratePkgContextInfo 输入 | `src/tasks/generate-pkg-context-info.js` | `pkg_context.rs` input snapshot |
| SDK loader and SDK functions | `src/sdk/*.js`, `load-sdk-functions.js` | `sdk.rs` |
| PathInfo build dirs | `src/tasks/sync/ide/*.js`, build dir const | `artifact_layout.rs` |
| invalid cache inputs | `invalid-cache-plugin.js` | `fingerprint.rs` |

## 目标目录

```text
src/
  lib.rs
  builder.rs
  project.rs
  module.rs
  package.rs
  dependency_graph.rs
  resolver_index.rs
  profile_store.rs
  sdk.rs
  resource_inventory.rs
  source_store.rs
  artifact_layout.rs
  fingerprint.rs
  context.rs
  plan_input.rs
  diagnostics.rs
```

## 文件作用和内部结构

- `builder.rs`: `BuildSessionBuilder`，唯一启动发现入口。按 project -> config -> modules -> packages -> SDK -> inventories 构建。
- `project.rs`: `ProjectSnapshot`，project root、selected product、build profile、app json、signing raw inputs。
- `module.rs`: `ModuleSnapshot`，module root、module json、source roots、resource roots、profile refs、target metadata。
- `package.rs`: `PackageSnapshot`，self/local/remote package roots、package manifest、acquisition result。
- `dependency_graph.rs`: `DependencyGraphSnapshot`、typed nodes/edges/topo waves、HAR/HSP/ByteCode/runtimeOnly 分类。
- `resolver_index.rs`: 从 graph + source roots + SDK snapshot 生成 `ResolverIndex`。
- `profile_store.rs`: profile 文件发现、parse cache、router/share/startup/privacy/forms fingerprints。
- `sdk.rs`: `SdkSnapshot`，tool paths、schema paths、ets-loader compile plugin metadata、support extensions。
- `resource_inventory.rs`: resource roots、rawfile、res profile、native/resource side input inventory。
- `source_store.rs`: source file inventory、source text/bytes cache、source fingerprint。
- `artifact_layout.rs`: hvigor-compatible intermediates/outputs layout，内部使用 `arkdown-path`。
- `fingerprint.rs`: `InputFingerprintSet`，覆盖 manifest/profile/package/SDK/resource/source/dependency graph。
- `context.rs`: `ModuleCompileContext`，`Arc<BuildSession>` + module/target/dependency view/artifact layout/fingerprint view。
- `plan_input.rs`: 给 CLI `BuildPlanBuilder` 的 narrow views。
- `diagnostics.rs`: 启动阶段诊断聚合。

## 串联关系

```text
arkdown-cli::BuildArgs
  -> BuildSessionBuilder
  -> BuildSession
  -> ModuleCompileContext[]
  -> arkdown-cli::BuildPlanBuilder
  -> domain plans and plugin constructors
```

## 迁移要点

- 初版不能是骨架，必须读真实 hvigor 输入。
- 禁止零指纹占位。
- session startup 可并行读 manifests/profile/resource/source/SDK metadata，但最终 merge 必须 deterministic。
- profile/resource/pack/sign/cache/abc 不得被 session 依赖。

