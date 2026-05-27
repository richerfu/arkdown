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

| Decorator           | Field backing class                         | `updateStateVars` slot? |
| ------------------- | ------------------------------------------- | ----------------------- |
| `@State`            | `ObservedPropertySimplePU<T>`               | No                      |
| `@Prop`             | `SynchedPropertySimpleOneWayPU<T>`          | Yes                     |
| `@Link`             | `SynchedPropertySimpleTwoWayPU<T>`          | No                      |
| `@StorageLink`      | `AppStorageLinkedProperty<T>`               | No                      |
| `@StorageProp`      | `AppStorageProperty<T>`                     | No                      |
| `@LocalStorageLink` | `LocalStorageLinkedProperty<T>`             | No                      |
| `@LocalStorageProp` | `LocalStorageProperty<T>`                   | Yes                     |
| `@Provide`          | `ObservedPropertyObjectPU<T>`               | No                      |
| `@Consume`          | `ObservedPropertyObjectPU<T>` (via context) | No                      |
| `@ObjectLink`       | `SynchedPropertyNesedObjectPU<T>`           | No                      |
| `@Watch`            | Adds `__watch_<field>` subscription call    | —                       |
| `@Builder`          | Standalone function (struct method → fn)    | —                       |
| `@BuilderParam`     | `WrappedBuilder<Parameters<T>>`             | Yes                     |

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
