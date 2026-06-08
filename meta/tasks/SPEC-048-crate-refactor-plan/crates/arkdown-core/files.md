# File API Plan: crates/arkdown-core

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 保留当前 `graph.rs`、`chunk.rs` 中的 dense ID/graph 思路，但把 build/profile/schema/path/plugin 内容迁出。
- 删除或迁出当前 `plugin.rs` 的 `ArkdownPlugin` legacy API 到删除目标 `arkdown-loader` 的迁移链，生产插件改用 rolldown。
- 当前 `manifest.rs` 中 raw `ModuleJson5`、`OhPackageJson5`、profile schema 迁到 `arkdown-ohos-schema`。
- 当前 `paths.rs::lexical_normalize` 迁到 `arkdown-path::normalize`。
- 当前 `config.rs` 中 `ArkdownConfig`、`SdkConfig`、`BuildOption` 等归一化配置迁到 `arkdown-config`。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | 无新增 domain 类型 | `pub use error::{ArkdownError, Result}`；re-export `ids`, `artifact`, `diagnostic`, `source_maps` narrow API | 当前 `lib.rs` re-export 收窄 | foundation crate，无直接 hvigor 文件 |
| `error.rs` | `ArkdownError`, `ArkdownErrorKind`, `Result<T>` | `ArkdownError::new(kind, message)`, `with_path`, `with_source`, `with_diagnostic_code` | 当前 `error.rs::ArkdownError` 扩展 | hvigor logger/error adaptor 的 typed 替代 |
| `ids.rs` | `ProjectId`, `ModuleId`, `TargetId`, `PackageId`, `DependencyNodeId`, `SourceFileId`, `ResourceFileId`, `ProfileFileId`, `CompileUnitId`, `ChunkId`, `PackEntryId` | `*_id::new`, `index`, `from_usize`; 实现 `oxc_index::Idx` | 当前 `graph.rs::ModuleIdx`, `chunk.rs::ChunkIdx` | hvigor task/module/dependency identity，替代字符串 key |
| `module.rs` | `ModuleKind`, `ArtifactKind`, `BuildMode`, `CompileMode`, `TargetName`, `ProductName` | `ModuleKind::from_hvigor_type`, `ArtifactKind::extension`, `CompileMode::as_hvigor_str` | 当前 `config.rs::OhosModuleType`, `CompileMode`, `BuildMode` | hvigor `ModuleType`, `CompileModeEnum`, package output extension |
| `graph.rs` | `DependencyEdgeKind`, `DependencyScope`, `TopoWave<T>`, `GraphDiagnostic`, `TopoSorter` | `TopoSorter::new(nodes, edges).topo_waves()`, `TopoSorter::validate_acyclic()` | 当前 `graph.rs::ModuleGraph` 保留纯图算法，dependency graph 移 session | hvigor `DependencyManager` 的 topo 结果表达 |
| `artifact.rs` | `ArtifactKind`, `ArtifactMeta`, `StageOutputKind`, `StageOutputMeta` | `ArtifactMeta::new`, `StageOutputMeta::for_kind` | 当前 `artifact.rs` 扩展 | hvigor intermediates/outputs 元数据 |
| `diagnostic.rs` | `Diagnostic`, `DiagnosticSeverity`, `DiagnosticCode`, `DiagnosticAttachment` | `Diagnostic::error`, `warning`, `with_path`, `with_note` | 从多 crate ad hoc error strings 合并 | hvigor `OhosLogger.printErrorExit` typed 替代 |
| `source_maps.rs` | `HvigorSourceMapScope`, `HvigorSourceMapOptions`, `HvigorSourceMapEntry`, `RecordNameParts`, `HvigorSourceMapWriter` | `HvigorSourceMapScope::from_record_name()`, `HvigorSourceMapOptions::from_bundle()`, `HvigorSourceMapWriter::new(options).to_json()` | 保留当前 `source_maps.rs` | `arkts-pack.js::getOutputConfig`、release map/sourceMaps.map 逻辑 |

## 必删/迁出文件

| 当前文件 | 处理 | 目标 |
| --- | --- | --- |
| `plugin.rs` | 删除 legacy `ArkdownPlugin`、hook args/output/cache context | `arkdown` + rolldown plugin trait；plugin crate adapter |
| `paths.rs` | 删除 | `arkdown-path::normalize` |
| `manifest.rs` | raw schema 迁出，只保留真正 domain-neutral artifact metadata | `arkdown-ohos-schema`, `arkdown-profile` |
| `config.rs` | 迁出 | `arkdown-config` |
| `resolution.rs` | package context/brief info 迁出到 resolver/session/profile plan；core 只留纯 ID/metadata 如确有共用 | `arkdown-resolver`, `arkdown-session` |

