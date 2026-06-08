# File API Plan: crates/arkdown-path

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `arkdown-path` 已有 `AbsPath`、`RelPath`、`ArtifactPath`、`ModuleSpecifier`、`OhmUrl`、serializer；继续扩成唯一 path policy crate。
- 从 `arkdown-core::paths`、`arkdown-utils`、resolver/profile/pack/CLI 中迁入 slash conversion、lexical normalize、`oh_modules` 分类、artifact path 构造。
- 对应 hvigor 的 `PathInfo`、`BuildDirConst`、restool/package zip entry 逻辑，但 API 必须是 typed path，不复制 JS 字符串拼接。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | 无新增 | narrow exports: `AbsPath`, `RelPath`, `ArtifactPath`, `ModuleSpecifier`, `OhmUrl`, `ArtifactPathSerializer`, `PathClassifier` | 当前 `lib.rs` 收窄 | path foundation |
| `abs.rs` | `AbsPath`, `AbsPathBuf`, `AbsPathError` | `AbsPath::new_existing`, `new_lexical`, `join`, `parent`, `as_path`, `into_path_buf`; `lexical_normalize_pathbuf` | 当前 `abs.rs`; `arkdown-core::paths::lexical_normalize` | hvigor existing input canonicalize boundary |
| `rel.rs` | `RelPath`, `RelPathBuf`, `RelPathError` | `RelPath::parse`, `from_path`, `join_to`, `as_path`, `into_path_buf` | 当前 `rel.rs`; CLI/profile relative path helpers | source/resource/profile relative roots |
| `artifact.rs` | `ArtifactPath`, `ArtifactPathBuf`, `ArtifactPathError` | `ArtifactPath::parse_slash`, `from_components`, `join`, `as_str`, `into_string` | 当前 `artifact.rs`; pack zip entry helpers | package-tool entry paths、loader/source map slash paths |
| `serializer.rs` | `ArtifactPathSerializer`, `SerializerError`, `SerializationRoot` | `serialize_relative`, `serialize_absolute_for_hvigor`, `zip_entry`, `loader_json_path`, `source_map_path`, `pkg_context_path` | 当前 serializer；profile `to_absolute_path_string`; pack `ArchivePath` | `generate-loader-json.js`, package zip/restool slash outputs |
| `specifier.rs` | `ModuleSpecifier`, `SpecifierKind`, `SpecifierError` | `ModuleSpecifier::new`, `kind`, `is_relative`, `is_bare`, `as_str`, `resolve_relative_text` | 当前 `specifier.rs`; resolver specifier classifier | `plugins/resolve/index.js` import specifier parse |
| `ohm.rs` | `OhmUrl`, `OhmSchema`, `OhmUrlError`, `OhmUrlParts` | `OhmUrl::new`, `from_package_module`, `parse`, `as_str`; `OhmUrlParts::to_url` | 当前 `ohm.rs`; resolver `ohm_url.rs` | `use-new-ohm-url-plugin.js`, `hspNameOhmMap` |
| `normalize.rs` | `NormalizeMode`, `PathNormalizer` | `PathNormalizer::new(mode)`, `PathNormalizer::lexical_normalize()`, `PathNormalizer::strip_relative_prefix()`, `PathNormalizer::normalize_existing_or_lexical()`, `PathNormalizer::normalize_output_path()` | 当前 `normalize.rs`; core/utils normalize | hvigor path resolve with non-existing output dirs |
| `classify.rs` | `PathKind`, `ModulesPathKind`, `PathClassifier` | `classify_path`, `is_inside_oh_modules`, `is_app_scope`, `classify_modules_path`, `package_components` | 当前 `classify.rs`; resolver `contains("oh_modules")` sites | OHPM package root classification |
| `display.rs` | `PathDisplay<'a>` | `PathDisplay::new`, `fmt` | 新增 | hvigor diagnostics path rendering |
| `compat.rs` | `HvigorPathCompat` | `HvigorPathCompat::to_slash()` only for audited compatibility, `HvigorPathCompat::canonicalize_or_owned()` deprecated then deleted | 当前 `compat.rs`; `arkdown-utils` helpers | temporary hvigor parity bridge |

## 禁止保留

- `String` 作为 path identity。
- inline `.replace('\\', "/")`。
- path substring package classification。
- `.to_string_lossy()` 参与 resolver/cache/dependency graph key。

