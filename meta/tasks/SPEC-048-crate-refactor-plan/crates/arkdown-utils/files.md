# File API Plan: crates/arkdown-utils

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

Action: Delete.

## 现有文件迁移

| 当前文件/API | 新位置 | 目标结构体/函数 |
| --- | --- | --- |
| `src/lib.rs` path helper | `arkdown-path::{abs,rel,normalize,serializer}` | `AbsPath`, `RelPath`, `lexical_normalize`, `ArtifactPathSerializer` |
| `src/lib.rs` slash conversion | `arkdown-path::serializer` | `ArtifactPathSerializer::zip_entry`, `loader_json_path` |
| `src/lib.rs` hash helper | `arkdown-cache::key` or `arkdown-session::fingerprint` | `CacheKey`, `InputFingerprintSet` |
| `src/lib.rs` generic fs helper | owning stage crate | stage-specific reader/writer |

## 删除函数合同

- Any function that reads/writes filesystem must move to owning stage object.
- Any pure path manipulation must move to `arkdown-path`.
- Any cache/hash function must move to `arkdown-cache` or session fingerprint.

## 对应上游逻辑

hvigor 的 `src/utils/*` 不是 arkdown 的目标结构。arkdown 必须按 ownership 拆分，不保留 utils crate。

