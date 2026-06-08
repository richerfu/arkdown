# File API Plan: crates/arkdown-obfuscate

## API 导出硬约束

- 本文件表格中的“方法/关联函数”列只能表示对应结构体、枚举或 trait 的 `impl` 方法、associated function、builder 方法或 trait 方法；不得规划 `pub fn` free function 作为 crate 对外 API。
- 旧代码中的 free function 只能作为“迁移来源”出现。迁移时必须挂到拥有上游语义的类型上，例如 `*Builder`、`*Planner`、`*Resolver`、`*Compiler`、`*Processor`、`*Writer`、`*Store`、`*Snapshot`、`*Policy`、`*Pass`、`*Runner`。
- 如果某个旧 free function 确实是跨 crate 共享的纯算法或基础转换，不能留在当前 domain crate 导出；必须迁入已有 common/foundation owner crate（例如 `arkdown-core`、`arkdown-path`、`arkdown-ohos-schema`、`arkdown-abc` 等按语义归属的 crate），并同样以命名类型的 associated method 暴露。SPEC-048 当前不新增泛化 `arkdown-common` crate。
- 每一行的结构体和方法必须对齐上游原始逻辑：`对应上游逻辑` 列必须写到 hvigor/SDK/arkdown-abc 的具体 task、plugin、class、函数或文件；若是 Rust ownership adapter，必须注明它替代的上游全局状态、工具函数或生命周期 hook。
- `lib.rs` 只能做窄 re-export，不得新增业务 free function。

## 迁移总则

- 当前 `Obfuscator`、rules/minifier/rename 可以保留，但 `compute_rule_hash(rule_files)` 的文件发现由 session/plan 提供。
- 对应 hvigor `ArkCompile.getObfuscationOptions`、HAR `ProcessObfuscationFiles`、SDK `generateObfuscationFile`。

## 目标文件 API

| 文件 | 结构体/枚举/类型 | 方法/关联函数 | 当前迁移来源 | 对应上游逻辑 |
| --- | --- | --- | --- | --- |
| `lib.rs` | re-export `Obfuscator`, `ObfuscationPlan`, `ObfuscationOutput` | `Obfuscator::new(plan).obfuscate()`, `ObfuscationPlan::should_obfuscate()` | current `lib.rs` narrowed | obfuscation typed facade for SDK terser/arkguard branches |
| `error.rs` | `ObfuscationError`, `ObfuscationDiagnostic` | `ObfuscationError::rule`, `parse`, `transform` | split current enum | obfuscation errors |
| `plan.rs` | `ObfuscationPlan`, `ObfuscationRuleInput`, `ConsumerObfuscationPlan`, `ObfuscationOutputPaths` | `from_build_options`, `validate`, `rule_fingerprint` | new from current `ObfuscationOptions` | `getObfuscationOptions`, `computeObfuscationFileHash` |
| `rules.rs` | `ObfuscationRules`, `RuleSection`, `RuleMergeResult` | `ObfuscationRules::parse`, `merge`, `is_reserved`, `from_plan` | current `rules.rs` | obfuscation config resolver |
| `transform.rs` | `ObfuscationOutput`, `SourcePatch`, `ObfuscationTransform` | `transform_source`, `apply_patches`, `collect_renames` | current `transform.rs` | source obfuscation |
| `rename.rs` | `NameAllocator`, `RenameMap` | `NameAllocator::new`, `next`, `reserve`, `allocate_for` | current `rename.rs` | name minification |
| `minifier.rs` | `MinifyOptions` | `minify_source`, `minify_with_options` | current `minifier.rs` | `terserPlugin` parity |
| `consumer.rs` | `ConsumerObConfigWriter`, `ConsumerObConfigOutput` | `generate_consumer_ob_config`, `write_consumer_rules` | new | `load-sdk-functions.js::generateObfuscationFile`, `process-obfuscation-files.js` |

