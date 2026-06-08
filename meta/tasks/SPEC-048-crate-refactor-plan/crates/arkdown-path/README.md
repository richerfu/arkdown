# crates/arkdown-path

Action: Add/Keep.

## 目标职责

`arkdown-path` 拥有所有路径域模型、lexical normalization、路径分类、case-exact 检查、artifact slash serialization、OHM URL 和 module specifier 处理。任何 crate 不得再手写 slash 替换、`contains("oh_modules")`、`starts_with('/')`、lossy path identity。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| `PathInfo` 派生 build/intermediates/outputs | `src/tasks/sync/ide/ohos-module-model-bean.js`, `src/const/build-directory-const.js` | `artifact.rs`, `layout names` |
| resolver 中 OHM URL/new OHM URL | `plugins/use-new-ohm-url-plugin.js`, `plugins/resolve/index.js` | `ohm.rs`, `specifier.rs` |
| package root 和 `oh_modules` 分类 | `src/project/dependency/*.js`, resolver plugin | `classify.rs` |
| restool/package zip entry slash path | `src/builder/restool-*.js`, package tasks | `serializer.rs` |
| case sensitive check | `use-sensitive-check-plugin.js` | `case_check` policy in `normalize.rs` or resolver integration |

## 目标目录

```text
src/
  lib.rs
  abs.rs
  rel.rs
  artifact.rs
  serializer.rs
  specifier.rs
  ohm.rs
  normalize.rs
  classify.rs
  display.rs
  compat.rs
```

## 文件作用和内部结构

- `abs.rs`: `AbsPath`、`AbsPathBuf`，只在 path boundary canonicalize 已存在输入。
- `rel.rs`: `RelPath`、`RelPathBuf`，用于 source root/profile/resource 相对路径。
- `artifact.rs`: `ArtifactPath`、`ArtifactPathBuf`，只表示产物内部 slash path，不表示 filesystem path。
- `serializer.rs`: `ArtifactPathSerializer`，唯一允许把 filesystem/path newtype 转成 slash string 的位置。
- `specifier.rs`: `ModuleSpecifier`、specifier parse/normalize，处理 import 文本而不是文件路径。
- `ohm.rs`: `OhmUrl`、normalized OHM URL builder、HSP/HAR package URL 表达。
- `normalize.rs`: lexical normalize、existing input canonicalize boundary、output lexical normalize。
- `classify.rs`: component-based `oh_modules`、`node_modules`、SDK path、project/module/resource path 分类。
- `display.rs`: `PathDisplay`，仅用于诊断。
- `compat.rs`: hvigor parity helper，只保留已审计的兼容序列化策略。

## 串联关系

```text
arkdown-path
  -> arkdown-config
  -> arkdown-resolver
  -> arkdown-session::ArtifactLayout
  -> domain plan paths
  -> artifact serializers
```

## 迁移要点

- `arkdown-utils` 的 path helper 迁入后删除旧 helper。
- resolver、profile、pack、resource、CLI 中所有 artifact path string assembly 迁到 serializer。
- 任何 cache key 中的 path identity 必须用 normalized typed path 加 content fingerprint。

