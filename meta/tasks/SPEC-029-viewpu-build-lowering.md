# SPEC-029: ViewPU Build() Body Lowering

**Crate**: `arkdown-plugin-ets-transform`
**Priority**: P1 (critical — blocks all real ArkUI pages)
**Blocked by**: SPEC-004a
**Upstream refs**:
- DevEco SDK: `<OHOS_SDK_HOME>/ets/build-tools/ets-loader/`
- `ets-loader/src/ui_syntax_checker.ts`
- `ets-loader/src/fast_build/ark_compiler/`
- `load-sdk-functions.js::loadSdkFunctions()` — `t.etsTransform()` invocation
- hvigor gap analysis: TASK-ETS-01, gap F16

---

## Background

ArkUI components are written as TypeScript `struct` declarations with a `build()` method
containing a declarative UI DSL. The `ets-loader` SDK plugin transforms this DSL into
imperative ViewPU API calls that the ArkRuntime component framework understands.

Without this lowering, any real ArkUI page (any file with `@Entry @Component struct`) compiles
syntactically but produces non-functional runtime code. The entire UI layer of every OHOS app
depends on this transform.

The current `arkdown-plugin-ets-transform` stubs out `build()` bodies and transforms state
decorators but does not perform ViewPU lowering. This SPEC defines the full lowering.

---

## ArkUI DSL → ViewPU Lowering Contract

### Input: ArkUI Component Struct

```typescript
@Entry
@Component
struct Index {
  @State message: string = 'Hello World';

  build() {
    RelativeContainer() {
      Text(this.message)
        .id('HelloWorld')
        .fontSize(50)
        .fontWeight(FontWeight.Bold)
    }
    .height('100%')
    .width('100%')
  }
}
```

### Output: ViewPU API Calls

```javascript
class Index extends ViewPU {
  constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
    super(parent, __localStorage, elmtId, extraInfo);
    if (typeof paramsLambda === "function") {
      this.paramsGenerator_ = paramsLambda;
    }
    this.__message = new ObservedPropertySimple('Hello World', this, "message");
    this.setInitiallyProvidedValue(params);
    this.finalizeConstruction();
  }

  setInitiallyProvidedValue(params) {
    if (params.message !== undefined) {
      this.message = params.message;
    }
  }

  updateStateVars(params) {}

  purgeVariableDependenciesOnElmtId(rmElmtId) {
    this.__message.purgeDependencyOnElmtId(rmElmtId);
  }

  aboutToBeDeleted() {
    this.__message.aboutToBeDeleted();
    SubscriberManager.Get().delete(this.id__());
    this.aboutToBeDeletedInternal();
  }

  get message() {
    return this.__message.get();
  }
  set message(newValue) {
    this.__message.set(newValue);
  }

  initialRender() {
    this.observeComponentCreation2((elmtId, isInitialRender) => {
      RelativeContainer.create();
      RelativeContainer.height('100%');
      RelativeContainer.width('100%');
    }, RelativeContainer);
    {
      this.observeComponentCreation2((elmtId, isInitialRender) => {
        Text.create(this.message);
        Text.id('HelloWorld');
        Text.fontSize(50);
        Text.fontWeight(FontWeight.Bold);
      }, Text);
      Text.pop();
    }
    RelativeContainer.pop();
  }

  rerender() {
    this.updateDirtyElements();
  }
}
```

---

## Lowering Rules

### 1. Struct → Class Extension

Every `@Component struct Foo` becomes `class Foo extends ViewPU`.

`@Entry @Component struct` additionally registers the component as an entry point:
`registerNamedRoute(() => new Foo(undefined, {}), "", { bundleName: "...", moduleName: "...", pagePath: "...", pageFullPath: "...", integratedHsp: false })`.

### 2. State Variable Lifting

Each decorated field is lifted to a private `__fieldName` backing property:

| Decorator | Backing type | Getter/setter |
|---|---|---|
| `@State` | `ObservedPropertySimple<T>` | get/set via `.get()`/`.set()` |
| `@Prop` | `SynchedPropertySimpleOneWay<T>` | get/set via `.get()`/`.set()` |
| `@Link` | `SynchedPropertySimpleTwoWay<T>` | get/set via `.get()`/`.set()` |
| `@ObjectLink` | `SynchedPropertyNesedObject<T>` | get only |
| `@Provide` | `ObservedPropertySimple<T>` + `provide_` map | get/set |
| `@Consume` | `initializeConsume()` call | get/set |
| `@StorageLink` | `AppStorageLinkedProperty<T>` | get/set |
| `@StorageProp` | `AppStorageLinkedProperty<T>` | get only |
| `@LocalStorageLink` | `LocalStorageLinkedProperty<T>` | get/set |
| `@LocalStorageProp` | `LocalStorageLinkedProperty<T>` | get only |

### 3. Constructor Generation

The constructor signature is always:
```javascript
constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo)
```

Body:
1. `super(parent, __localStorage, elmtId, extraInfo)`
2. `if (typeof paramsLambda === "function") { this.paramsGenerator_ = paramsLambda; }`
3. One backing property initialization per state variable
4. `this.setInitiallyProvidedValue(params)`
5. `this.finalizeConstruction()`

### 4. setInitiallyProvidedValue Generation

For each `@State`/`@Prop` field `foo`:
```javascript
if (params.foo !== undefined) { this.foo = params.foo; }
```

### 5. build() → initialRender() + observeComponentCreation2

The `build()` method body is lowered into `initialRender()`. Each UI component call in the
build body is wrapped in `this.observeComponentCreation2((elmtId, isInitialRender) => { ... }, ComponentClass)`.

**Component call pattern** (container components):
```
ComponentName(args) { children }  .attr1(v1) .attr2(v2)
```
becomes:
```javascript
this.observeComponentCreation2((elmtId, isInitialRender) => {
  ComponentName.create(args);
  ComponentName.attr1(v1);
  ComponentName.attr2(v2);
}, ComponentName);
// children lowered recursively
ComponentName.pop();
```

**Leaf component pattern** (no children):
```
Text(this.message) .fontSize(50)
```
becomes:
```javascript
this.observeComponentCreation2((elmtId, isInitialRender) => {
  Text.create(this.message);
  Text.fontSize(50);
}, Text);
Text.pop();
```

**Attribute chain ordering**: attribute calls on the parent appear **before** children are
lowered; `.pop()` appears **after** all children.

### 6. rerender() Method

Always emitted as:
```javascript
rerender() {
  this.updateDirtyElements();
}
```

### 7. updateStateVars() Method

Always emitted. In PU / partial-update mode this follows SDK
`ets-loader/lib/process_component_member.js`: `validatePropDecorator()` adds a
`createStateVarsBody()` statement, which directly calls `reset(params.x)`.
```javascript
updateStateVars(params) {
  this.__propField.reset(params.propField);
}
```

`@State` state-vars paths, when emitted for lazy/advanced modes, keep the SDK
truthiness guard before `.set(...)`.

### 7a. Button Create Method

SDK `process_component_build.js::createComponent()` special-cases `Button`:

- `Button(label)` without children lowers to `Button.createWithLabel(label)`.
- `Button(label) { ... }` lowers to `Button.createWithChild(label)`.
- Other built-in components use `.create(...)`.

### 7b. Builder Parent Context

SDK `parentConditionalExpression()` is `(parent ? parent : this)`. Top-level
`@Builder` functions and component `@Builder` methods receive `parent = null`;
their lowered ArkUI framework calls use that parent context:

```javascript
(parent ? parent : this).observeComponentCreation2(...)
(parent ? parent : this).ifElseBranchUpdateFunction(...)
```

### 8. purgeVariableDependenciesOnElmtId()

For each state variable:
```javascript
this.__fieldName.purgeDependencyOnElmtId(rmElmtId);
```

### 9. aboutToBeDeleted()

For each state variable:
```javascript
this.__fieldName.aboutToBeDeleted();
```
Plus:
```javascript
SubscriberManager.Get().delete(this.id__());
this.aboutToBeDeletedInternal();
```

---

## Architecture

### Transform Pass Location

The ViewPU lowering runs as a new transform pass inside `arkdown-plugin-ets-transform`,
after the existing decorator transforms (SPEC-004a) and before codegen.

```rust
// crates/arkdown-plugin-ets-transform/src/lib.rs
pub struct EtsTransformPlugin {
    config: EtsTransformConfig,
}

impl ArkdownPlugin for EtsTransformPlugin {
    async fn transform(&self, ctx: &TransformContext, code: &str, id: &str) -> PluginResult<Option<TransformedCode>> {
        if !id.ends_with(".ets") {
            return Ok(None);
        }
        let mut result = transform_decorators(code, id)?;  // existing: @State, @Prop, etc.
        result = transform_viewpu(result, id, &self.config)?;  // new: build() lowering
        Ok(Some(result))
    }
}
```

### ViewPU Transform Entry Point

```rust
// crates/arkdown-plugin-ets-transform/src/viewpu/mod.rs

pub struct ViewPuConfig {
    pub bundle_name: String,
    pub module_name: String,
    pub page_path: String,     // relative path from src/main/ets/
}

/// Lower all @Component struct declarations in `source` to ViewPU class form.
/// Returns the transformed source as a String.
pub fn transform_viewpu(
    source: &str,
    file_path: &str,
    config: &ViewPuConfig,
) -> Result<String, ViewPuError>;
```

### Internal Structure

```
crates/arkdown-plugin-ets-transform/src/viewpu/
    mod.rs          — entry point, visitor dispatch
    component.rs    — struct → class transformation
    state_vars.rs   — backing property generation per decorator
    build_body.rs   — build() → initialRender() lowering
    constructor.rs  — constructor + lifecycle method generation
    entry.rs        — @Entry registerNamedRoute emission
```

### AST Visitor Strategy

Use `ohos-rs/oxc` (pinned rev) to parse the `.ets` source and walk the AST:

1. Collect all `struct` declarations with `@Component` decorator.
2. For each component struct:
   a. Collect state variable fields (decorated properties).
   b. Collect the `build()` method body.
   c. Emit the transformed class using `oxc_codegen`.
3. Replace the original struct declaration with the generated class in the output.

---

## Public API

```rust
// crates/arkdown-plugin-ets-transform/src/viewpu/mod.rs

#[derive(Debug, Clone)]
pub struct ViewPuConfig {
    pub bundle_name: String,
    pub module_name: String,
    pub page_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ViewPuError {
    #[error("parse error in {file}: {message}")]
    ParseError { file: String, message: String },
    #[error("unsupported ArkUI construct: {0}")]
    Unsupported(String),
}

pub fn transform_viewpu(
    source: &str,
    file_path: &str,
    config: &ViewPuConfig,
) -> Result<String, ViewPuError>;
```

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
|---|---|---|
| `transform_viewpu()` in `transform` hook | rolldown `Plugin::transform` | Same hook; different transform content |
| oxc AST visitor for struct→class | Babel `@babel/plugin-transform-class-properties` | Conceptually similar; ArkUI-specific output |
| `ViewPuConfig` from loader.json | rolldown `InputOptions` | Config sourced from loader.json fields |

---

## Test Requirements

### Basic Component

- Input: `@Entry @Component struct Index { build() { Text('hello') } }`
- Assert: output contains `class Index extends ViewPU`
- Assert: output contains `initialRender()`
- Assert: output contains `observeComponentCreation2`
- Assert: output contains `Text.create('hello')`
- Assert: output contains `Text.pop()`
- Assert: output contains `rerender() { this.updateDirtyElements(); }`

### State Variable Lifting

- Input: `@State message: string = 'Hello'`
- Assert: output contains `this.__message = new ObservedPropertySimple('Hello', this, "message")`
- Assert: output contains `get message() { return this.__message.get(); }`
- Assert: output contains `set message(newValue) { this.__message.set(newValue); }`

### Attribute Chain

- Input: `Text(this.msg).fontSize(50).fontWeight(FontWeight.Bold)`
- Assert: output contains `Text.create(this.msg)` before `Text.fontSize(50)` before `Text.fontWeight(FontWeight.Bold)` before `Text.pop()`

### Nested Children

- Input: `Column() { Text('a') Text('b') }`
- Assert: output contains `Column.create()` before both `Text.create('a')` and `Text.create('b')`, and `Column.pop()` after both `Text.pop()` calls.

### @Entry Registration

- Input: `@Entry @Component struct MyPage { build() {} }`
- Assert: output contains `registerNamedRoute` call with correct `bundleName`, `moduleName`, `pagePath`.

### No-Op for Non-Component Files

- Input: a `.ets` file with no `@Component` decorator
- Assert: output is identical to input (no transform applied)

### Multiple Components in One File

- Input: two `@Component struct` declarations in the same file
- Assert: both are independently transformed; neither interferes with the other.

### Prop Decorator

- Input: `@Prop count: number`
- Assert: backing type is `SynchedPropertySimpleOneWay`
- Assert: `setInitiallyProvidedValue` contains the prop update
- Assert: `updateStateVars` contains direct `this.__count.reset(params.count)` with no `undefined` guard

### Lifecycle Preservation

- Input: component with `aboutToAppear()` method
- Assert: `aboutToAppear()` method is preserved in the output class unchanged.

### Source Map Continuity

- Assert: the transform emits source map entries mapping generated ViewPU calls back to the
  original `.ets` source positions (per-statement granularity minimum).
