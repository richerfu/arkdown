# SPEC-004a: ArkTS Transform (Pure Rust ets-loader Port)

**Crate**: `arkdown-plugin-ets-transform`
**Priority**: P0
**Blocked by**: SPEC-001, SPEC-004
**Blocks**: SPEC-005

---

## Summary

Pure Rust port of the OpenHarmony SDK's `ets-loader` ArkTS transform step. Replaces the
subprocess invocation described in SPEC-004 (`transform.rs`). Implements three components
as a single `ArkdownPlugin` crate:

| Component      | SDK equivalent        | What it does                                       |
| -------------- | --------------------- | -------------------------------------------------- |
| `etsTransform` | `etsTransform` plugin | Desugars ArkUI decorators and `struct` components  |
| `etsChecker`   | `etsChecker` plugin   | Validates decorator usage rules (diagnostics only) |
| `apiTransform` | `apiTransform` plugin | Rewrites OHOS API calls by target SDK version      |

When this crate is in use, the `transform.rs` subprocess path in `arkdown-loader` is
disabled. `OHOS_SDK_HOME` is no longer required at build time.

---

## Plugin Interface

The crate exports a single public type:

```rust
pub struct EtsTransformPlugin {
    pub config: Arc<ArkdownConfig>,
}
```

It implements `ArkdownPlugin`. Only the `transform` hook is non-trivial; all others are
no-ops (default implementations).

The `transform` hook:

1. Returns `Ok(None)` immediately for any `module_type != ModuleType::Ets`
2. Parses source with `ohos-rs/oxc` (ArkTS grammar)
3. Runs `etsChecker` — emits diagnostics, never aborts on warnings
4. Runs `etsTransform` — mutates AST in place
5. Runs `apiTransform` — rewrites API calls by `config.sdk_config.compile_api_version`
6. Runs `oxc_codegen` — produces TypeScript text + source map
7. Returns `ModuleSource` with `module_type` changed to `ModuleType::Ts`

---

## etsTransform: ArkUI Decorator Desugaring

### Overview

ArkTS struct-based components are desugared to class-based TypeScript. The generated
class extends `ViewPU` (stage model, API ≥ 9) and uses the ArkUI state management
runtime types.

### @Component struct → ViewPU class

Input:

```typescript
@Component
struct Counter {
  @State count: number = 0

  build() {
    Column() {
      Text(`${this.count}`)
        .onClick(() => { this.count++ })
    }
  }
}
```

Generated output:

```typescript
class Counter extends ViewPU {
  private __count: ObservedPropertySimplePU<number>;

  get count(): number {
    return this.__count.get();
  }
  set count(v: number) {
    this.__count.set(v);
  }

  constructor(
    parent: ViewPU,
    params: Partial<{ count: number }>,
    localStorage?: LocalStorage,
    elmtId: number = -1,
    paramsLambda?: () => object,
    extraInfo?: ExtraInfo,
  ) {
    super(parent, localStorage, elmtId, extraInfo);
    this.__count = new ObservedPropertySimplePU<number>(0, this, "count");
    this.updateWithValueParams(params);
    this.finalizeConstruction();
  }

  updateStateVars(params: Partial<{ count: number }>): void {
    // no @Prop fields, so empty
  }

  purgeVariableDependenciesOnElmtId(rmElmtId: number): void {
    this.__count.purgeDependencyOnElmtId(rmElmtId);
  }

  aboutToBeDeleted(): void {
    this.__count.aboutToBeDeleted();
    SubscriberManager.Get().delete(this.id__());
    this.aboutToBeDeletedInternal();
  }

  initialRenderFunction(): void {
    this.observeComponentCreation2((elmtId, isInitialRender) => {
      Column.create();
    }, Column);
    this.observeComponentCreation2((elmtId, isInitialRender) => {
      Text.create(`${this.count}`);
      Text.onClick(() => {
        this.count++;
      });
    }, Text);
    Text.pop();
    Column.pop();
  }

  rerender(): void {
    this.updateDirtyElements();
  }
}
```

### State property class mapping

Backing class names and constructor init patterns are derived from ets-loader
`process_component_class.js` (`createTypeReferencePU`) and
`process_component_member.js` (`updateObservedPropertyPU` etc.) for PU / partial-update
mode (API >= 9).

| Decorator           | Field backing class (`private __x: …`)  | Constructor init expression                                  | `updateStateVars` slot? |
| ------------------- | ---------------------------------------- | ------------------------------------------------------------ | ----------------------- |
| `@State`            | `ObservedPropertySimplePU<T>`            | `new ObservedPropertySimplePU(init, this, "x")`              | No                      |
| `@Prop`             | `SynchedPropertySimpleOneWayPU<T>`       | `new SynchedPropertySimpleOneWayPU(params.x, this, "x")`     | Yes                     |
| `@Link`             | `SynchedPropertySimpleTwoWayPU<T>`       | `new SynchedPropertySimpleTwoWayPU(params.x, this, "x")`     | No                      |
| `@StorageLink`      | `ObservedPropertyAbstractPU<T>`          | `this.__x = undefined` (storage runtime wires it up)         | No                      |
| `@StorageProp`      | `ObservedPropertyAbstractPU<T>`          | `this.__x = undefined` (storage runtime wires it up)         | No                      |
| `@LocalStorageLink` | `ObservedPropertyAbstractPU<T>`          | `this.__x = undefined` (storage runtime wires it up)         | No                      |
| `@LocalStorageProp` | `ObservedPropertyAbstractPU<T>`          | `this.__x = undefined` (storage runtime wires it up)         | Yes                     |
| `@Provide`          | `ObservedPropertySimplePU<T>`            | `new ObservedPropertySimplePU(init, this, "x")`              | No                      |
| `@Consume`          | `ObservedPropertyAbstractPU<T>`          | `this.initializeConsume("alias_or_x", "x")`                  | No                      |
| `@ObjectLink`       | `SynchedPropertyNesedObjectPU<T>`        | `new SynchedPropertyNesedObjectPU(params.x, this, "x")`      | No                      |
| `@Watch`            | (no backing field; adds `declareWatch`)  | `this.declareWatch("x", this.handlerName)` after field inits | —                       |
| `@Builder`          | Struct method lowered like `build()` body | —                                                            | —                       |
| `@BuilderParam`     | `WrappedBuilder<Parameters<T>>`          | `this.x = init` (direct, no `__` prefix in ctor)            | Yes                     |

> **Note on simple vs object variants**: The ets-loader picks between `Simple` and `Object`
> backing class variants based on an `isSimpleType` check (whether the declared type is
> `string`, `number`, or `boolean`).  The pure-Rust transform defaults to the `Simple`
> variants for all types as a conservative choice.  A future enhancement can add type
> heuristics to select `Object` variants for non-primitive types.

### @Entry

`@Entry` is always paired with `@Component`. Additional generated code:

```typescript
// Appended after the class definition:
ViewStackProcessor.StartGetAccessRecordingFor(Counter);
Counter.create = function (value?: { count?: number }) {
  /* ... */
};
ViewStackProcessor.StopGetAccessRecording();
```

Also registers the component in `__ARKUI_ENTRY__` if it is the page root.

### build() method desugaring

The `build()` method body is a DSL where function calls are ArkUI component factories.
Each top-level call `Foo(args)` is rewritten to a pair:

```typescript
// DSL: Foo(args) { children }
this.observeComponentCreation2((elmtId, isInitialRender) => {
  Foo.create(args);
}, Foo);
// ... desugared children ...
Foo.pop();
```

Method chains on builder calls (`.onClick(...)`, `.width(...)`, etc.) stay as attribute
calls inside the `observeComponentCreation2` callback:

```typescript
this.observeComponentCreation2((elmtId, isInitialRender) => {
  Text.create(`${this.count}`);
  Text.onClick(() => {
    this.count++;
  });
}, Text);
```

Leaf components (no closing brace children) still get `.pop()` but emit no nested
`observeComponentCreation2` calls.

### @Builder functions

Struct methods decorated with `@Builder` are lifted out to standalone functions bound to `this`:

```typescript
// Input:
@Builder myBuilder() { Text('hi') }

// Output (outside class, generated as class method):
myBuilder(): void {
  this.observeComponentCreation2((elmtId, isInitialRender) => {
    Text.create('hi');
  }, Text);
  Text.pop();
}
```

When a `@Builder` is called from `build()`, it is called as `this.myBuilder()`.

---

## etsChecker: Validation Rules

Runs before the transform. Produces `Vec<Diagnostic>` (warnings or errors). Never panics
or aborts the build on warnings; fatal errors surface as `ArkdownError::EtsCheck`.

| Rule                         | Severity | Condition                                                                    |
| ---------------------------- | -------- | ---------------------------------------------------------------------------- |
| `missing-build-method`       | Error    | `@Component struct` has no `build()` method                                  |
| `entry-without-component`    | Error    | `@Entry` applied to a struct without `@Component`                            |
| `build-return-type`          | Error    | `build()` has a non-void return type annotation                              |
| `state-on-non-component`     | Error    | `@State`/`@Prop`/`@Link` on a struct without `@Component`                    |
| `unsupported-state-type`     | Warning  | `@State` field type is not a supported observable type (function, undefined) |
| `duplicate-component-name`   | Error    | Two `@Component struct` definitions with the same name in one file           |
| `worker-uses-ui-decorator`   | Error    | File in `workers/` uses `@State`, `@Component`, or `@Entry`                  |
| `builder-param-missing-type` | Warning  | `@BuilderParam` field has no type annotation                                 |

---

## apiTransform: SDK API Rewrites

Rewrites deprecated or version-gated OHOS API calls based on
`ArkdownConfig.sdk_config.compile_api_version`.

### Rules

| Target API Version | Rewrite                                                          |
| ------------------ | ---------------------------------------------------------------- |
| < 9                | `import * as X from '@ohos/Y'` → `import X from '@system.Y'`     |
| ≥ 9                | No rewrite needed; `@ohos/*` and `@kit.*` are the canonical form |

All API rewrites are driven by a static version table baked into the crate (derived from
`sdk/default/openharmony/ets/api/` declaration structure). No runtime SDK path lookup.

---

## Source Map Chain

The transform produces a source map from the generated TypeScript back to the original
`.ets` source. This source map is stored in `ModuleSource.source_map` and is chained
with the downstream source maps from `oxc_codegen` and `es2abc` to produce the final
`.hap`-embedded source map.

---

## Files to Create

```
crates/arkdown-plugin-ets-transform/
├── Cargo.toml
└── src/
    ├── lib.rs               (EtsTransformPlugin: ArkdownPlugin impl)
    ├── error.rs             (EtsTransformError, Diagnostic, DiagnosticSeverity)
    ├── transform/
    │   ├── mod.rs           (EtsTransformer: entry point, dispatch to sub-transforms)
    │   ├── component.rs     (@Component struct → ViewPU class skeleton)
    │   ├── decorator.rs     (state property mapping: @State/@Prop/@Link/etc.)
    │   ├── entry.rs         (@Entry: create() factory + __ARKUI_ENTRY__ registration)
    │   ├── build_fn.rs      (build() body desugaring → observeComponentCreation2 pairs)
    │   └── builder.rs       (@Builder/@BuilderParam handling)
    ├── checker/
    │   ├── mod.rs           (check() entry point → Vec<Diagnostic>)
    │   └── rules.rs         (all validation rules listed above)
    └── api_transform/
        ├── mod.rs           (apply() entry point)
        └── version_table.rs (static API version rewrite rules)
```

---

## Cargo.toml Dependencies

```toml
[dependencies]
arkdown-core = { path = "../arkdown-core" }

# Parser, transformer, codegen — ohos-rs fork only
oxc_allocator  = { workspace = true }
oxc_parser     = { workspace = true }
oxc_ast        = { workspace = true }
oxc_codegen    = { workspace = true }
oxc_span       = { workspace = true }

# Async trait
async-trait    = { workspace = true }
tokio          = { workspace = true }

# Error handling
thiserror      = { workspace = true }
```

---

## Relation to SPEC-004

SPEC-004 (`arkdown-loader`) currently specifies a subprocess fallback:

```
sdk/ets/build-tools/ets-loader/bin/ets-loader --input <file> --output <transformed.ts>
```

When `arkdown-plugin-ets-transform` is registered in the loader's plugin list (SPEC-004),
the `transform.rs` subprocess path is dead code and should be removed. Update
`arkdown-loader/src/transform.rs` to delegate entirely to the plugin chain rather than
invoking the SDK binary directly.

---

## Acceptance Criteria

- `cargo build -p arkdown-plugin-ets-transform` compiles with zero warnings
- `cargo clippy -p arkdown-plugin-ets-transform -- -D warnings` passes
- Unit tests (in `src/transform/component.rs`, `checker/rules.rs`) cover each decorator type
- The `@Component` + `@State` + `build()` round-trip test produces output equivalent to
  what the SDK ets-loader emits for the same input
- `OHOS_SDK_HOME` is not read at any point in the crate

---

## Design References

- `meta/design/pipeline.md` — Phase 4 (Transform)
- `meta/design/plugins.md` — plugin inventory (no dedicated entry; ets-loader was a subprocess)
- `meta/tasks/SPEC-004-module-loader.md` — defines the `transform` hook and plugin chain
- `meta/design/types.md` — `ModuleSource`, `ArkdownPlugin` trait, `ArkdownConfig`

---

## Rolldown Alignment

| arkdown concept                              | rolldown equivalent                                                            | note                                                                                                                         |
| -------------------------------------------- | ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| `EtsTransformPlugin::transform`              | `Plugin::transform` returning `HookTransformOutput { code, map, module_type }` | Same hook; arkdown changes `module_type` from `Ets` to `Ts` in the output                                                    |
| `etsTransform` (ArkUI desugar)               | Babel/SWC decorator transforms in the rollup/rolldown ecosystem                | Functionally equivalent: AST rewrite that desugars custom syntax to standard TS; arkdown uses `ohos-rs/oxc` instead of Babel |
| `etsChecker`                                 | rolldown/Vite `vite-plugin-checker` pattern                                    | Diagnostic emission without aborting; rolldown does not have built-in type checking                                          |
| `apiTransform` (SDK API rewrite)             | Babel `@babel/plugin-transform-*` plugins                                      | Version-gated rewrite table; no rolldown equivalent                                                                          |
| Source map from ETS → TS                     | `HookTransformOutput::map`                                                     | Identical: the transform produces a source map that is chained with downstream maps                                          |
| `ModuleSource.module_type` change `Ets → Ts` | `HookTransformOutput::module_type` field                                       | rolldown allows transforms to change the module type; arkdown does the same                                                  |

**Key divergence:** This transform runs only on `ModuleType::Ets` files. There is no equivalent in rolldown — ArkTS is OHOS-specific. The entire `etsTransform` stage has no counterpart in standard JS bundlers.

---

## Public API

```rust
/// Single exported type — registered in the plugin list by arkdown-loader.
pub struct EtsTransformPlugin {
    pub config: Arc<ArkdownConfig>,
}

impl ArkdownPlugin for EtsTransformPlugin {
    fn name(&self) -> &str;
    async fn transform(&self, ctx: &TransformContext, source: ModuleSource)
        -> PluginResult<Option<ModuleSource>>;
    // All other hooks: default no-op
}
```

No other public types are exported. `EtsTransformer`, `EtsChecker`, and `ApiTransformer` are crate-private.

---

## Test Requirements

- `@Component struct Counter` with `@State count: number` round-trip: transform output contains `class Counter extends ViewPU` with `__count: ObservedPropertySimplePU<number>`
- `@Entry` on a component generates `ViewStackProcessor.StartGetAccessRecordingFor` after the class definition
- `@Prop` field appears in `updateStateVars` method; `@State` field does not
- `@Builder myBuilder()` method is preserved as a class method (not lifted to standalone function) and its `build()`-style body is desugared
- `build()` with `Column { Text("hi") }` → two `observeComponentCreation2` calls with `Column.pop()` and `Text.pop()`
- Method chains in `build()` (`.onClick(...)`) appear inside the `observeComponentCreation2` callback
- `etsChecker` rule `missing-build-method`: `@Component struct Foo {}` (no build) → `Err(ArkdownError::EtsCheck)` with rule ID `missing-build-method`
- `etsChecker` rule `entry-without-component`: `@Entry struct Foo {}` (no `@Component`) → error
- `etsChecker` rule `worker-uses-ui-decorator`: file in `workers/` with `@State` → error
- `apiTransform` for API version < 9: `import * as X from '@ohos/Y'` → `import X from '@system.Y'`
- `apiTransform` for API version >= 9: no rewrite applied
- Transform returns `Ok(None)` immediately for `module_type != ModuleType::Ets` without parsing
- Source map is present in `ModuleSource.source_map` after transform (not `None`)
- `OHOS_SDK_HOME` is not read at any point — confirmed by test that sets `OHOS_SDK_HOME=""` and asserts no error
