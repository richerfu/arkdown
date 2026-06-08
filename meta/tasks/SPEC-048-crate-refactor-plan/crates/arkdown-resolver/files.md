# File API Plan: crates/arkdown-resolver

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 保留当前 `OhosResolver` 解析能力，但它的 constructor 改为接收 `ResolverIndex`，不再读 package files。
- 当前 `oh_modules.rs::OhPackage`/metadata 如果是 raw package metadata，应由 session/ohos-schema 供给；resolver 只保留 resolved package view。
- `case_check` 从 plugin 迁入 resolver boundary。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | `ResolveRequest`, `ResolveResult` | exports `OhosResolver`, `ResolverIndex` | 当前 `lib.rs` 扩展 | resolver crate boundary |
| `error.rs` | `ResolveError`, `ResolveDiagnostic` | `ResolveError::unresolved`, `case_mismatch`, `ambiguous_package` | 当前 `error.rs` | rollup resolver errors |
| `resolver.rs` | `OhosResolver`, `ResolverConfig`, `PackageRoot`, `PackageCandidate` | `OhosResolver::new(index, config)`, `resolve`, `resolve_relative`, `resolve_bare`, `resolve_sdk`, `resolve_package_entry`, `resolve_with_extensions` | 当前 `resolver.rs` 保留并去 IO | `plugins/resolve/index.js` |
| `index.rs` | `ResolverIndex`, `PackageIndexEntry`, `SourceRootEntry`, `ExtensionSet`, `SdkModuleEntry` | `ResolverIndex::builder`, `package_by_name`, `package_for_path`, `source_roots`, `sdk_modules` | 从 current `PackageRootIndex` 抽出并由 session 构建 | `arkts-pack.js::getResolveModules` |
| `specifier.rs` | `SpecifierClass` | `classify_specifier`, `to_module_specifier` | 当前 `specifier.rs`; bridge to `arkdown-path` | import specifier classification |
| `oh_modules.rs` | `ResolvedPackage`, `OhModulesLayout`, `PackageOwnership` | `package_root_in_oh_modules`, `owner_for_path`, `entry_candidates` | 当前 `oh_modules.rs` 去 file read | OHPM resolver package ownership |
| `ohm_url.rs` | `OhmUrlBuilder`, `OhmUrlParseResult` | `generate_ohm_url`, `parse_ohm_url`, `build_for_package` | 当前 `ohm_url.rs`; use `arkdown-path::OhmUrl` | `use-new-ohm-url-plugin.js` |
| `sdk.rs` | `SdkResolveIndex` | `sdk_api_dir`, `sdk_kits_dir`, `resolve_sdk_module` | 当前 `sdk.rs` | SDK api/kits modules |
| `case_check.rs` | `CaseCheckResult`, `CaseCheckPolicy` | `check_path_case`, `check_case_impl`, `validate_resolved_case` | 当前 resolver `case_check.rs` + plugin sensitive-check | `use-sensitive-check-plugin.js` |

## 删除/迁出来源

| 当前逻辑 | 新位置 |
| --- | --- |
| package json read/walk-up in resolver/plugin | `arkdown-session::ResolverIndexBuilder` |
| plugin `case_check.rs` | `arkdown-resolver::case_check` |
| `ResolvedImportMap` mutation inside plugin | typed fact output in plugin, resolver only returns ownership |

