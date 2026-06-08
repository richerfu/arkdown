# File API Plan: crates/arkdown-ohos-schema

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 只存 raw schema 和 parse helpers，不做 normalized policy。
- 当前 `arkdown-config::build_profile`, `module_json`, `oh_package` 的 raw structs 迁入本 crate。
- 当前 `arkdown-core::manifest` 中 raw module/app/profile/package schema 迁入本 crate。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `SchemaParser`, `RawSchemaFileKind`, raw schema types | `SchemaParser::json5().parse<T>()`, `SchemaParser::json().parse<T>()` | 新增 parser type；现有 raw structs re-export | hvigor `Json5Reader.getJson5Obj` typed parser replacement |
| `build_profile.rs` | `RawBuildProfile`, `RawApp`, `RawBuildProfileProduct`, `RawBuildProfileModule`, `RawBuildProfileTarget`, `RawBuildProfileSigningConfig`, `RawSigningMaterial`, `RawBuildOption`, `RawStrictMode`, `RawArkOptions`, `RawResOptions`, `RawSourceOption`, `RawRuntimeOnly` | `RawBuildProfile::from_json5_str`, `targets_for_module_raw` | 当前 `arkdown-config::build_profile::*` | `src/options/build/*.js`, `BuildProfileTask` |
| `app_json.rs` | `RawAppJson5`, `RawAppJson5App`, `RawAppIcon`, `RawAppLabel` | `RawAppJson5::from_json5_str` | 当前 `arkdown-config::RawAppJson5`, core manifest | `AppScope/app.json5` |
| `module_json.rs` | `RawModuleJson`, `RawModuleEntry`, `RawAbility`, `RawExtensionAbility`, `RawSkill`, `RawMetadata`, `RawRequestPermission`, `RawDependency` | `RawModuleJson::from_json5_str`, `module_type_raw`, `profile_refs` | 当前 `arkdown-config::module_json`, `arkdown-core::manifest` | `module.json5`, `ProcessProfile` |
| `oh_package.rs` | `RawOhPackage`, `RawDependencyMap`, `RawPackageMetadata`, `RawRuntimeOnly`, `RawDynamicImport` | `RawOhPackage::from_json5_str`, `dependency_names`, `runtime_only_names` | 当前 `arkdown-config::oh_package`, profile runtime-only helpers | `oh-package.json5`, `ProcessOhPackageJson` |
| `loader_json.rs` | `RawLoaderJson`, `RawCompileEntry`, `RawPkgBriefInfo`, `RawByteCodeHarInfo`, `RawDynamicImportLibInfo` | `RawLoaderJson::from_json_str`, `to_json_value` | Current `arkdown-profile::LoaderJson` serialization fields as raw mirror | `GenerateLoaderJson`, `arkts-pack.js::loadAceBuildJson` |
| `pack_info.rs` | `RawPackInfo`, `RawPackInfoSummary`, `RawPackInfoApp`, `RawPackInfoVersion`, `RawPackInfoModuleSummary`, `RawPackInfoPackage` | `RawPackInfo::from_json_str`, `package_names` | current schema file | `MakePackInfo` |
| `signing.rs` | `RawSigningConfig`, `RawProvisionProfile`, `RawBundleInfo`, `RawProfileValidity` | `RawProvisionProfile::from_json`, `bundle_name`, `profile_type` | current schema file + sign crate parsing | `ValidateSign`, signing profile |
| `profiles/router.rs` | `RawRouterMap`, `RawRouteEntry` | `RawRouterMap::from_json5_str`, `entries_sorted` | current schema file/profile router logic | `ProcessRouterMap`, `router-map-plugin.js` |
| `profiles/share.rs` | `RawShareProfile`, `RawShareTarget` | `RawShareProfile::from_json5_str` | current schema file | `ProcessShareConfig` |
| `profiles/startup.rs` | `RawStartupProfile`, `RawStartupTask` | `RawStartupProfile::from_json5_str`, `task_names` | current schema file | `ProcessStartupConfig` |
| `profiles/privacy.rs` | `RawPrivacyProfile` | `RawPrivacyProfile::from_json5_str`, `is_enabled` | current schema file/profile privacy | `pac-process/*.js` |
| `profiles/forms.rs` | `RawFormProfile`, `RawFormEntry` | `RawFormProfile::from_json5_str`, `form_names` | current schema file/profile forms | form profile handling |
| `profiles/shortcut.rs` | `RawShortcutProfile`, `RawShortcutEntry` | `RawShortcutProfile::from_json5_str` | current schema file | `process-shortcut-profile.js` |
| `profiles/distro_filter.rs` | `RawDistroFilterProfile` | `RawDistroFilterProfile::from_json5_str` | current schema file | `process-distro-filter.js` |
| `profiles/permission_reason.rs` | `RawPermissionReasonProfile`, `RawPermissionEntry` | `RawPermissionReasonProfile::from_json5_str` | current schema file | `process-permission-reason` |

## 禁止保留

- schema crate 不做 path resolve。
- schema crate 不做 product/target selection。
- schema crate 不做 dependency graph classification。

