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
6. Generates source map via `oxc_codegen` from the **kit-rewritten** pre-transform source
   (not the final ViewPU class) so positions reference the short original `.ets` file
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
| `@PropRef`          | `SynchedPropertySimpleOneWayPU<T>`       | `new SynchedPropertySimpleOneWayPU(params.x, this, "x")`     | Yes                     |
| `@StoragePropRef`   | `ObservedPropertyAbstractPU<T>`          | `this.__x = undefined` (storage runtime wires it up)         | No                      |
| `@LocalStoragePropRef` | `ObservedPropertyAbstractPU<T>`       | `this.__x = undefined` (storage runtime wires it up)         | No                      |
| `@Local`            | `ObservedPropertySimplePU<T>`            | `new ObservedPropertySimplePU(init, this, "x")`              | No                      |
| `@Param` / `@Once`  | `SynchedPropertySimpleOneWayPU<T>`       | `new SynchedPropertySimpleOneWayPU(params.x, this, "x")`     | Yes                     |
| `@Provider`         | `ObservedPropertySimplePU<T>`            | `new ObservedPropertySimplePU(init, this, "x")`              | No                      |
| `@Consumer`         | `ObservedPropertyAbstractPU<T>`          | `this.initializeConsume("alias_or_x", "x")`                  | No                      |
| `@Env`              | `ObservedPropertySimplePU<T>`            | `new ObservedPropertySimplePU(init, this, "x")`              | No                      |
| `@Event` / `@Require` | (regular member metadata)              | decorator stripped, member emitted as TypeScript             | —                       |
| `@Watch`            | (no backing field; adds `declareWatch`)  | `this.declareWatch("x", this.handlerName)` after field inits | —                       |
| `@Computed` / `@Monitor` / lifecycle decorators | (regular method metadata) | decorator stripped, method emitted as TypeScript | —                       |
| `@Builder`          | ✅ Class method, body lowered via `lower_build_body_or_raw` | —                                         | —                       |
| `@Styles`           | ✅ `private` class method, `.attr(x)` → `this.attr(x)` in body | —                                     | —                       |
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

✅ **Implemented** (`crates/arkdown-plugin-ets-transform/src/transform/`, 2026-05-29)

Struct methods decorated with `@Builder` remain as class methods with the same
name (they are NOT renamed to `__method__`). Top-level `@Builder export function`
is emitted as an exported `function name(...): void`, matching upstream handling
of builder functions before bundling. The decorator is stripped.

The body is lowered via `lower_build_body_or_raw`:

1. **If the OXC ETS parser emits `ArkUIComponentExpression` nodes** (expected
   in future parser versions that apply ArkUI rules to `@Builder` bodies as
   well as `build()`): the body is fully desugared into
   `observeComponentCreation2` + `.pop()` pairs — identical to `build()`.

2. **Current OXC ETS behaviour**: the parser can expose ArkUI children inside
   canonical `build()` bodies, nested callbacks, and some builder bodies. When a
   builder body is parsed as regular TypeScript, the fallback emits statement
   text as TypeScript instead of guessing missing ArkUI AST.

```typescript
// Input:
@Builder myBuilder() { Text('hi') }

// Output (current — raw body fallback):
myBuilder(): void {
  Text('hi')
}

// Output (future — full ArkUI desugaring when parser supports it):
myBuilder(): void {
  this.observeComponentCreation2((elmtId, isInitialRender) => {
    Text.create('hi');
  }, Text);
  Text.pop();
}
```

When a `@Builder` is called from `build()`, it is called as `this.myBuilder()`.

### @Styles functions

✅ **Implemented** (`crates/arkdown-plugin-ets-transform/src/transform/`, 2026-05-29)

Struct methods decorated with `@Styles` become `private` class methods with
the decorator stripped.  The attribute chain body is lowered: lines starting
with `.` are rewritten to `this.attr(x)` calls (the implicit receiver is made
explicit).

```typescript
// Input:
@Styles myStyle() { .width(100).height(50) }

// Output:
private myStyle(): void {
  this.width(100);
  this.height(50);
}
```

The `lower_styles_body` function is text-based: it reads the raw source span
of the method body, strips the surrounding `{}`/`}`, and rewrites each
`.attr(x)` line.  This works regardless of whether the OXC ETS parser produces
special ArkUI attribute expression nodes for `@Styles` bodies.

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

The transform produces a source map anchored to the **pre-transform** source, not to the
generated ViewPU/TypeScript output.  Specifically:

- For `.ets` files (struct components): the source map is generated by `oxc_codegen` from
  the **kit-rewritten** `.ets` source (after `@kit.*` import expansion, before struct
  desugaring).  This keeps source positions in the map pointing to the short original
  struct source (15 lines) rather than the expanded ViewPU class (100+ lines).  hvigor's
  `ets-loader` generates a similarly sparse map via its struct-position tracking.

- For `.ts` files: the source map is generated from the **kit-rewritten** TypeScript
  source (before the version-gated `@ohos` rewrite).  The expanded individual imports
  give `oxc_codegen` more lines to generate mapping segments for.

This source map is forwarded to rolldown as `HookTransformOutputMap::Sourcemap` and then
used by `write_hvigor_source_maps` in the CLI to build the final `sourceMaps.map` file.

**Important**: The final `sourceMaps.map` is written by `write_hvigor_source_maps` as
pretty-printed JSON with 2-space indentation to match hvigor's output format.

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

**Key divergence:** this transform is OHOS-specific and has no standard rolldown
equivalent. arkdown runs it in both `load` and `transform`: `load` lowers ArkTS-only
syntax before rolldown's TypeScript parser, while `transform` preserves the existing
source-map chaining path for already-loadable ETS/TS modules.

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
- ✅ `@Builder myBuilder()` method is preserved as a class method (not lifted to standalone function); decorator stripped; body lowered via `lower_build_body_or_raw` (ArkUI desugaring or raw fallback); output is valid TypeScript
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
