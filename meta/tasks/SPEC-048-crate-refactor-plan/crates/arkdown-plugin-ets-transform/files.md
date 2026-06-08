# File API Plan: crates/arkdown-plugin-ets-transform

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 本 crate 只保留 rolldown adapter。当前除 `plugin/hook` 外的大部分 transform/checker/api/resource/module rewrite 代码迁到 `arkdown-ets-transform`。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `EtsTransformPlugin`, `EtsTransformPluginPlan` | `EtsTransformPlugin::new(plan)` | current `lib.rs` reduced | SDK `etsTransform/apiTransform` plugin adapter facade |
| `plugin.rs` | `EtsTransformPlugin`, `EtsTransformPluginOptions`, `SdkPluginOrderSlot` | `new`, `with_context`, `hook_usage`, `order_before_api_transform`, `order_before_gen_abc` | current plugin entry | SDK `compile_plugin.js` adapter |
| `hook.rs` | `TransformHookInput`, `TransformHookOutput`, `HookTransformDecision`, `RolldownTransformMeta` | `load_hook_if_needed`, `transform_hook`, `classify_hook_input`, `to_rolldown_output`, `merge_transform_facts` | current transform hook logic | Rollup hooks for `visualTransform/etsTransform/apiTransform` |
| `options.rs` | `EtsTransformPluginPlan`, `PluginFilePlan`, `PluginSideOutputPlan` | `from_module_context`, `transform_options_for_id`, `visual_options_for_id`, `api_options_for_id` | new from session plan | `arkts-pack.js::getBuiltInPlugins`, SDK plugin config |

## Hook 分流

| 输入 | hook 行为 |
| --- | --- |
| `.ets` | 调用 `arkdown-ets-transform` 的 visual prepass、checker、ArkUI lowering、API transform。是否返回 JS 由 `TransformOutput` 决定。 |
| `.ts` | 调用 checker/TS transform/import path/kit/API transform；不跑 `.ets` 专属 visual 和 `validateEts`。 |
| `.js` | 只在 crossplatform、widget 或上游等价的 API/libso/lazy import 分支中调用 API transform；不调用 ArkUI lowering。 |
| `.json` | 不处理，交给 bundler/`arkdown-ts2abc` module source。 |
| `.d.ts/.d.ets` | 直接跳过；声明文件由 `arkdown-ets-transform::declaration` 或 pack/HAR 阶段处理。 |

插件 crate 不允许读取 `.visual`、SDK config、resource table、profile 或 package 文件。`load` hook 只能接收 bundler 已加载源码，并从 `EtsTransformPluginPlan` 取对应 snapshot/facts。

## 迁出清单

| 当前文件/目录 | 新位置 |
| --- | --- |
| `transform/*` | `arkdown-ets-transform::{component,viewpu,transform,...}` |
| `api_transform/*` | `arkdown-ets-transform::api_transform` |
| `checker/*` | `arkdown-ets-transform::checker` |
| `source_edit.rs`, `source_map.rs` | `arkdown-ets-transform::{source,source_map}` |
| `resource_transform.rs`, `module_rewrite.rs`, `module_emit.rs`, `global_rewrite.rs` | `arkdown-ets-transform` |
| `kit_map.rs`, `sys_resource.rs` | `arkdown-ets-transform::sdk_adapter` or session SDK snapshot |
