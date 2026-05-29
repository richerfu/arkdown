# SPEC-030: @Builder and @Styles Decorator Transform

**Crate**: `arkdown-plugin-ets-transform`
**Priority**: P1 (high — common ArkUI patterns used in most real apps)
**Blocked by**: SPEC-004a
**Can be developed in parallel with**: SPEC-029
**Upstream refs**:
- DevEco SDK: `<OHOS_SDK_HOME>/ets/build-tools/ets-loader/`
- `ets-loader/src/fast_build/ark_compiler/` — decorator transform passes
- hvigor gap analysis: TASK-ETS-02

---

## Background

`@Builder` and `@Styles` are ArkUI decorators that define reusable UI fragments and style
sets respectively. They are extremely common in real ArkUI applications — virtually every
non-trivial component uses one or both.

- `@Builder` functions desugar into builder lambda captures that integrate with ViewPU's
  component creation chain.
- `@Styles` functions desugar into attribute set functions that can be applied to components.

Both must integrate with the ViewPU output from SPEC-029. This SPEC covers their transform
rules and can be implemented in parallel with SPEC-029 since they operate at the decorator
level rather than the `build()` body level.

---

## @Builder Transform

### Member @Builder

A `@Builder` method inside a component struct:

```typescript
@Component
struct MyComp {
  @Builder
  MyButton(text: string) {
    Button(text)
      .fontSize(16)
  }

  build() {
    Column() {
      this.MyButton('Click me')
    }
  }
}
```

Transforms to a method that wraps the builder body in `observeComponentCreation2`:

```javascript
MyButton(text) {
  this.observeComponentCreation2((elmtId, isInitialRender) => {
    Button.create(text);
    Button.fontSize(16);
  }, Button);
  Button.pop();
}
```

The call site `this.MyButton('Click me')` in `initialRender()` is emitted as-is (the method
call is preserved; no additional wrapping at the call site).

### Global @Builder (module-level function)

A `@Builder` function at module level:

```typescript
@Builder
function GlobalButton(text: string) {
  Button(text).fontSize(16)
}
```

Transforms to a regular function with the same ViewPU lowering applied to the body:

```javascript
function GlobalButton(text) {
  (this as unknown as ViewPU).observeComponentCreation2((elmtId, isInitialRender) => {
    Button.create(text);
    Button.fontSize(16);
  }, Button);
  Button.pop();
}
```

### @BuilderParam (reference to @Builder)

A `@BuilderParam` field holds a reference to a `@Builder` function. The field type becomes
`() => void` (a builder lambda):

```typescript
@Component
struct Card {
  @BuilderParam content: () => void

  build() {
    Column() {
      this.content()
    }
  }
}
```

The `@BuilderParam` field is initialized in the constructor from `params.content` if provided,
defaulting to a no-op lambda:

```javascript
this.content = this.initializeBuilderParam("content",
  params.content !== undefined ? params.content : () => {});
```

The call site `this.content()` in `initialRender()` is emitted as-is.

---

## @Styles Transform

### Member @Styles

A `@Styles` method inside a component struct:

```typescript
@Component
struct MyComp {
  @Styles
  buttonStyle() {
    .fontSize(16)
    .padding(8)
    .backgroundColor(Color.Blue)
  }

  build() {
    Button('Click')
      .buttonStyle()
  }
}
```

The `@Styles` method body contains bare attribute calls (no receiver). These are transformed
into a method that takes an implicit `this` receiver and applies the attribute calls to the
current component:

```javascript
buttonStyle() {
  // Attribute calls become static calls on the current component context
  // The component context is tracked via the ViewPU attribute stack
  ViewPU.applyStyles(() => {
    // Each attribute call in the @Styles body
    __currentComponent__.fontSize(16);
    __currentComponent__.padding(8);
    __currentComponent__.backgroundColor(Color.Blue);
  });
}
```

**Note**: The exact desugaring of `@Styles` bare attribute calls depends on the ArkUI version.
In API 9+, `@Styles` methods are called directly on the component chain. The implementation
should match the output of `ets-loader` for the target API version.

The call site `.buttonStyle()` in the `build()` chain is preserved as a method call on the
component; it is lowered as part of the component's attribute chain in SPEC-029's `build()`
lowering.

### Global @Styles

A `@Styles` function at module level:

```typescript
@Styles
function globalButtonStyle() {
  .fontSize(16)
  .padding(8)
}
```

Transforms to a function with the same attribute-application semantics.

---

## Architecture

### Transform Pass Location

`@Builder` and `@Styles` transforms run as part of the existing decorator transform pipeline
in `arkdown-plugin-ets-transform`, before the ViewPU `build()` lowering (SPEC-029):

```
decorator transforms (SPEC-004a: @State, @Prop, ...)
    ↓
@Builder / @Styles transforms (this SPEC)
    ↓
ViewPU build() lowering (SPEC-029)
    ↓
codegen
```

### Internal Structure

```
crates/arkdown-plugin-ets-transform/src/
    builder.rs   — @Builder method/function transform
    styles.rs    — @Styles method/function transform
```

### Public API

```rust
// crates/arkdown-plugin-ets-transform/src/builder.rs

/// Transform @Builder methods and functions in the parsed AST.
/// Must be called after decorator collection and before ViewPU build() lowering.
pub fn transform_builders(
    program: &mut oxc_ast::ast::Program,
    component_ctx: &ComponentContext,
) -> Result<(), EtsTransformError>;

// crates/arkdown-plugin-ets-transform/src/styles.rs

/// Transform @Styles methods and functions in the parsed AST.
pub fn transform_styles(
    program: &mut oxc_ast::ast::Program,
    component_ctx: &ComponentContext,
) -> Result<(), EtsTransformError>;
```

Where `ComponentContext` is a shared context object built during the decorator collection
phase (SPEC-004a) that tracks which structs are `@Component` and which fields/methods carry
which decorators.

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
|---|---|---|
| `transform_builders()` in `transform` hook | rolldown `Plugin::transform` | Same hook |
| `@Builder` → builder lambda | Babel closure transform | Conceptually similar; ArkUI-specific output |
| `@Styles` → attribute set function | CSS-in-JS style injection | Different mechanism; ArkUI-specific |

---

## Test Requirements

### @Builder Member Method

- Input: `@Builder MyButton(text: string) { Button(text) }`
- Assert: output contains `MyButton(text)` method (not a `@Builder` decorated method)
- Assert: method body contains `observeComponentCreation2` wrapping `Button.create(text)`
- Assert: `@Builder` decorator is removed from the method

### @Builder Global Function

- Input: `@Builder function GlobalBtn(text: string) { Button(text) }`
- Assert: output is a plain function (no `@Builder` decorator)
- Assert: body contains `observeComponentCreation2` wrapping

### @BuilderParam Field

- Input: `@BuilderParam content: () => void`
- Assert: constructor contains `this.initializeBuilderParam("content", ...)` call
- Assert: `@BuilderParam` decorator is removed from the field

### @BuilderParam with Default

- Input: `@BuilderParam content: () => void = this.defaultContent`
- Assert: constructor uses the default builder reference if `params.content` is undefined

### @Styles Member Method

- Input: `@Styles buttonStyle() { .fontSize(16) }`
- Assert: output contains `buttonStyle()` method
- Assert: `@Styles` decorator is removed from the method
- Assert: method body applies attributes (exact form depends on API version)

### @Styles Global Function

- Input: `@Styles function globalStyle() { .padding(8) }`
- Assert: output is a plain function (no `@Styles` decorator)

### Call Site Preservation

- Input: `build() { Button('x').buttonStyle() }`
- Assert: `.buttonStyle()` call is preserved in the lowered `initialRender()` attribute chain

### No-Op for Files Without @Builder/@Styles

- Input: a `.ets` file with no `@Builder` or `@Styles` decorators
- Assert: output is identical to input (no transform applied by these passes)

### Integration with ViewPU Lowering (SPEC-029)

- Input: component with both `@State` fields and `@Builder` methods
- Assert: the full output (after both transforms) is valid JavaScript
- Assert: `@Builder` method body is correctly lowered within the class
- Assert: `@State` field backing properties are correctly generated
