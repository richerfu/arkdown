# crates/arkdown-pack

Action: Keep.

## 目标职责

`arkdown-pack` 负责 HAP/HSP/HAR archive construction、pack.info、integrated HSP/native/resource/artifact entry 收集。它消费 `PackagePlan`，不得 late-read module json、profile、oh-package、integrated HSP 列表。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| PackageHap | `src/tasks/package-hap.js`, `pack/pack-hap-request.js` | `hap.rs`, `packager.rs` |
| PackageHsp | `src/tasks/hsp/package-hsp.js`, `fast-package-hsp.js` | `hsp.rs` |
| PackageHar | `src/tasks/har/package-har.js`, `abstract/abstract-package-har.js` | `har.rs` |
| MakePackInfo | `src/tasks/make-pack-info.js`, `abstract-make-pack-info.js` | `pack_info.rs` |
| ProcessIntegratedHsp | `src/tasks/process-integrated-hsp.js` | `integrated_hsp.rs` |
| ProcessHarArtifacts | `src/tasks/har/process-har-artifacts.js` | `har_artifacts.rs` |
| packing tool options | `src/builder/inner-java-command-builder/packing-tool-options.js` | `tool_options.rs` |

## 目标目录

```text
src/
  lib.rs
  error.rs
  plan.rs
  packager.rs
  zip_writer.rs
  hap.rs
  hsp.rs
  har.rs
  pack_info.rs
  integrated_hsp.rs
  har_artifacts.rs
  native_libs.rs
  tool_options.rs
```

## 文件作用和内部结构

- `plan.rs`: `PackagePlan`、archive kind、ordered pack entries、tool mode、output path。
- `packager.rs`: stage facade，dispatch HAP/HSP/HAR。
- `zip_writer.rs`: deterministic zip writer，entry order from `PackEntryId`。
- `hap.rs`: HAP-specific entry layout。
- `hsp.rs`: HSP-specific entry layout and shared har/tgz relation。
- `har.rs`: HAR archive layout、declaration/resource/package metadata entries。
- `pack_info.rs`: pack.info writer/reader。
- `integrated_hsp.rs`: integrated HSP entries from session snapshot。
- `har_artifacts.rs`: HAR artifacts processing output。
- `native_libs.rs`: native SO entry ownership from dependency graph/native inventory。
- `tool_options.rs`: hvigor package-tool parity options。

## 串联关系

```text
profile/resource/bundle/abc outputs
  -> PackagePlan
  -> arkdown-pack
  -> unsigned HAP/HSP/HAR
  -> arkdown-sign
```

## 迁移要点

- 不读取 `module.json5` 或 generated module json，路径由 plan 给。
- zip entry slash path 只走 `arkdown-path::ArtifactPathSerializer`。
- pack entry discovery/hashing 可并行，archive write 顺序稳定。

