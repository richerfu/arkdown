# File API Plan: crates/arkdown-config

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- raw schema 迁入 `arkdown-ohos-schema`，本 crate 只做 normalize 和 SDK probing。
- 当前 `build_profile.rs` 的 raw structs 要删除或 re-export schema 类型，函数保留为 normalizer 方法。
- `resolve.rs::load_config` 拆为 session startup read + `ConfigNormalizer`。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | `NormalizedBuildConfig`, `NormalizedModuleConfig`, `NormalizedTargetConfig` | exports normalizer/sdk types only | 当前 `lib.rs`, `resolve.rs` | config domain |
| `error.rs` | `ConfigError`, `ConfigDiagnostic` | `ConfigError::missing_product`, `invalid_target`, `sdk_not_found` | 当前 `error.rs` | hvigor config validation errors |
| `normalize.rs` | `ConfigNormalizer`, `ConfigNormalizeInput`, `ConfigNormalizeOutput` | `ConfigNormalizer::new`, `normalize_project`, `normalize_module`, `normalize_target`, `select_product`, `select_build_mode` | 当前 `build_profile.rs::select_effective_target`, `pick_effective_raw_target` | `find-target-product.js`, build option managers |
| `build_options.rs` | `BuildOption`, `StrictModeConfig`, `ArkOptions`, `ObfuscationConfig`, `ResOptions`, `SourceOption`, `RuntimeOnlyConfig`, `SigningConfigRef` | `BuildOption::from_raw`, `merge_target_override`, `debuggable`, `case_sensitive_check` | 当前 `arkdown-core::config`, `arkdown-config::build_profile` | `src/options/build/*.js` |
| `product.rs` | `ProductConfig`, `ProductSelection`, `BundleVersion` | `ProductSelection::resolve`, `ProductConfig::from_raw` | 当前 `RawProduct` usage | product selection |
| `target.rs` | `TargetConfig`, `TargetSelection`, `DeviceTypeSet`, `RuntimeOs` | `TargetSelection::resolve`, `TargetConfig::from_raw`, `applicable_to_product` | current target selection functions | `ModuleInspection.collectTargetRuntimeOS` |
| `sdk.rs` | `SdkConfig`, `SdkComponentPaths`, `SdkMetadata`, `SdkProbe` | `find_sdk_root`, `sdk_config_from_root`, `discover_sdk`, `discover_compile_sdk_version`, `discover_sdk_api_version`, `discover_ark_vm_version` | 当前 `sdk.rs` 保留，读操作由 session 调用 | `src/sdk/sdk-util.js`, `default-sdk.js` |
| `diagnostics.rs` | `ConfigDiagnosticBuilder` | `runtime_os_mismatch`, `stage_api_too_low`, `invalid_module_type`, `signing_material_missing` | 新增，从 scattered errors 合并 | `module-inspection.js`, `validate-sign.js` |

## 删除/迁出来源

| 当前文件/API | 处理 |
| --- | --- |
| `RawBuildProfile`, `RawModule`, `RawTarget`, `RawAppJson5` | 迁到 `arkdown-ohos-schema` |
| `load_config(project_root)` | 拆成 `BuildSessionBuilder` 读取 + `ConfigNormalizer` |
| SDK file reads in later stages | 只能由 session startup 调用 sdk probe |

