# File API Plan: crates/arkdown-ohpm

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `arkdown-ohpm` 已有 lockfile/cache/registry/tarball/integrity/acquisition 雏形，继续补齐为 production package acquisition。
- 从 `arkdown-profile::dependency_graph` 中迁出 `ohpm_global_cache_root`、lockfile/global cache/registry 相关逻辑。
- 本 crate 不输出 dependency graph，只输出 acquired package root 和 metadata。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | `OhpmClientConfig` | exports `PackageName`, `PackageAcquirer`, `OhPackageLockfile` | 当前 `lib.rs` 扩展 | OHPM acquisition boundary |
| `error.rs` | `OhpmError`, `OhpmErrorKind` | `OhpmError::registry`, `cache`, `integrity`, `tarball` | 当前 `error.rs` | hvigor `OHPM_INSTALL_FAILED` diagnostics |
| `package_name.rs` | `PackageName`, `PackageNameError` | `PackageName::parse`, `scope`, `unscoped`, `as_str` | 当前 `package_name.rs` | OHPM scoped package names |
| `lockfile.rs` | `OhPackageLockfile`, `LockfileEntry`, `LockfilePackageRef` | `OhPackageLockfile::from_json5_str`, `from_path`, `entry_for`, `strip_version_suffix` | 当前 `lockfile.rs`; profile dependency graph lockfile reads | `oh-package-lock.json5` |
| `cache.rs` | `CacheHit`, `OhpmCacheLayout`, `CacheEntryId` | `ohpm_cache_root`, `cache_path`, `lookup_cache`, `lookup_via_lockfile`, `install_marker_path` | 当前 `cache.rs`; `arkdown-profile::dependency_graph::ohpm_global_cache_root` | hvigor OHPM target/cache root |
| `registry.rs` | `RegistryVersionMetadata`, `OhpmRegistryClient`, `RegistryRequest`, `RegistryResponse` | `OhpmRegistryClient::new`, `version_metadata`, `download_tarball`, `with_proxy` | 当前 `registry.rs` | hvigor plugin factory `ohpm i`, proxy env |
| `tarball.rs` | `TarballExtractOptions`, `ExtractedPackage` | `extract_tgz`, `safe_entry_path`, `strip_top_component`, `lexical_normalize` | 当前 `tarball.rs` | OHPM tarball install |
| `integrity.rs` | `IntegrityError`, `IntegrityAlgorithm` | `verify`, `parse_integrity`, `digest_matches` | 当前 `integrity.rs` | npm/OHPM dist integrity |
| `acquisition.rs` | `OhpmResolutionMode`, `PackageAcquisitionRequest`, `PackageAcquisitionSource`, `PackageAcquisitionResult`, `PackageAcquirer` | `PackageAcquirer::new`, `resolve`, `resolve_all`, `resolve_offline`, `resolve_prefer_offline`, `install_from_registry` | 当前 `OhpmResolver` rename/expand | plugin factory `ohpm i --all --target_path` |

## 删除/迁出来源

| 当前来源 | 迁移到 |
| --- | --- |
| `arkdown-profile/src/dependency_graph.rs::ohpm_global_cache_root` | `cache.rs::ohpm_cache_root` |
| profile graph 中 lockfile/version/cache lookup | `lockfile.rs`, `cache.rs`, `acquisition.rs` |
| 任何 tarball extraction helper | `tarball.rs` |

