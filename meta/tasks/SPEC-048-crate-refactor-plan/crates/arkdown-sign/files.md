# File API Plan: crates/arkdown-sign

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `HapSigner` 和 low-level signing code 可保留。
- build-profile signing config resolve/decrypt material path 由 config/session plan 提供，sign crate 只执行。
- `write_hvigorw_script` 不是签名 domain 核心，应迁到 CLI fixture/tooling 或删除生产导出。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `Signer`, `SignPlan`, `SignOutput` | `Signer::new(plan).sign()`, `Signer::sign_file()` | current `HapSigner` narrowed | `SignHap`, `SignHsp`, `PackageSignHar` typed facade |
| `error.rs` | `SignError`, `SignDiagnostic` | `SignError::material`, `keystore`, `zip`, `pkcs7` | current `SignError` | signing errors |
| `plan.rs` | `SignPlan`, `SignKind`, `SigningMaterial`, `SigningTool`, `SigningPassword` | `SignPlan::validate`, `input`, `output`, `material` | new from current signer args | `SignModel`, `CommonSignCommandBuilder` |
| `signer.rs` | `Signer`, `SignResult` | `Signer::new`, `sign`, `sign_hap`, `sign_hsp`, `sign_har` | split current `HapSigner` | `sign-hap.js`, `hsp/sign-hsp.js` |
| `remote_hsp.rs` | `RemoteHspSignPlan`, `RemoteHspSignEntry`, `RemoteHspSignOutput` | `from_dependency_graph`, `sign_project_remote_hsp`, `sign_module_remote_hsp`, `metadata_for_pkg_context` | new from remote HSP signing handling | `SignProjectRemoteHsp`, `SignModuleRemoteHsp`, `remote-hsp-utils.js` |
| `tool.rs` | `SignToolCommand`, `SignToolOptions` | `from_plan`, `to_args`, `execute` | new if external tool path used | `signing-options.js`, `new-signing-options.js` |
| `keystore.rs` | `SigningKey`, `KeyStore`, `KeyStoreEntry` | `SigningKey::from_pkcs12`, `load_keystore`, `decrypt_password` | current `keystore.rs`, `decrypt_deveco_password` | `DecipherUtil`, keystore material |
| `digest.rs` | `DigestAlgorithm`, `DigestOutput` | `compute_hap_digest`, `digest_zip_sections` | current `digest.rs` | signing digest |
| `pkcs7.rs` | `CmsSignedDataBuilder`, `Der` | `build_cms_signed_data`, DER encode helpers | current `pkcs7.rs` | PKCS7/CMS block |
| `signing_block.rs` | `SigningBlock`, `DigestPair` | `build_signing_block`, `encode_digest_pairs` | current `signing_block.rs` | HAP signing block |
| `zip.rs` | `ZipEntry`, `EocdRecord`, `HapZip` | `HapZip::parse`, `sections_for_digest`, `write_with_signing_block`, `find_eocd` | current `zip.rs` | zip signing |

## 必迁/必改

- `decrypt_deveco_password` 路径 resolution 由 plan 给，不在 sign crate 猜 project root。
- final signing sequential boundary 保留。
- remote HSP 的发现和版本选择来自 `arkdown-session::DependencyGraphSnapshot`，sign crate 只接收 `RemoteHspSignPlan` 并产出签名后路径/metadata。
