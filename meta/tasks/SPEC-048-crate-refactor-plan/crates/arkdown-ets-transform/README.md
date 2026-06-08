# crates/arkdown-ets-transform

Action: Add.

## 目标职责

`arkdown-ets-transform` 拥有 OXC-based ArkTS transform core。它不实现 rolldown hook，不读 project/package/profile/SDK 文件，不拥有 cache storage。它输入 source、transform options、SDK/component metadata snapshot，输出 transformed code、source map、declaration metadata、component facts。

## 上游对应

| hvigor 上游逻辑 | 上游文件 | 本 crate 对应 |
| --- | --- | --- |
| SDK `etsTransform`/`apiTransform`/visual transform | `load-sdk-functions.js`, SDK `compile_plugin.js` | `transform.rs`, `sdk_adapter.rs` |
| ArkTS component transform | SDK plugins from `loadSdkFunctions` | `component/*.rs`, `viewpu/*.rs` |
| declaration emit | HAR build declaration steps | `declaration.rs` |
| source map transform | `arkts-pack.js` output sourcemap | `source_map.rs` |
| UI transform optimization | `ark-compile.js::initDefaultArkCompileConfig` | `options.rs` |

## 目标目录

```text
src/
  lib.rs
  options.rs
  transform.rs
  source.rs
  source_map.rs
  diagnostics.rs
  sdk_adapter.rs
  component/
    mod.rs
    analysis.rs
    metadata.rs
    fields.rs
    decorators.rs
    build_fn.rs
  viewpu/
    mod.rs
    emit.rs
    finalize.rs
    source_map.rs
  declaration.rs
```

## 文件作用和内部结构

- `options.rs`: `EtsTransformOptions`，来自 session/build plan。
- `transform.rs`: `EtsTransformer`，每个 transform task 拥有自己的 OXC allocator。
- `source.rs`: source text abstraction，来自 hook input 或 source store。
- `source_map.rs`: transform source map builder。
- `diagnostics.rs`: parser/transform diagnostic。
- `sdk_adapter.rs`: SDK metadata 的 typed adapter，不动态读取 SDK 文件。
- `component/*`: component analysis、decorator、field、build function lowering。
- `viewpu/*`: ViewPU emit/finalize/source map 逻辑，从 plugin crate 迁入。
- `declaration.rs`: HAR declaration emit metadata。

## 串联关系

```text
arkdown-plugin-ets-transform
  -> arkdown-ets-transform::EtsTransformer
  -> transformed module code/source map
  -> rolldown transform output
```

## 迁移要点

- 从 `arkdown-plugin-ets-transform/src/transform/*` 移出 transform core。
- 不保存 OXC allocator-backed AST 到 long-lived cache/session。
- raw rewrite 必须转成 span patch，并 deterministic order 应用。

