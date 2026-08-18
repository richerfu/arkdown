# OXC #23043 AstBuilder Migration Guide (ohos-rs/oxc @ 3f2b4b7)

Scope: after upgrading oxc to `3f2b4b7d`, the deprecated `AstBuilder` methods
are **removed** (compile errors, not warnings). This document is the single
mapping spec every migration must follow.

## 1. Import changes

```rust
// OLD
use oxc_ast::{AstBuilder, NONE};
// NEW
use oxc_ast::builder::AstBuilder;   // no longer re-exported from crate root
// NONE is gone — use Option::None directly
```

`AstBuilder<'a>` is **no longer `Copy`**. Pass it by reference everywhere:

```rust
// OLD: fn foo(ast: AstBuilder<'b>, ...)
// NEW: fn foo(ast: &AstBuilder<'b>, ...)
```

At call sites that create one: `let ast = AstBuilder::new(allocator);` still
works; then pass `&ast`.

## 2. Arena primitive methods

| OLD | NEW |
|-----|-----|
| `ast.str(s)` | `Str::from_in(s, ast.allocator())` (also accepts `&str`, `String`, `Atom`) |
| `ast.vec()` | `ArenaVec::new_in(ast.allocator())` |
| `ast.vec1(x)` | `ArenaVec::from_value_in(x, ast.allocator())` |
| `ast.vec_from_iter(it)` | `ArenaVec::from_iter_in(it, ast.allocator())` |
| `ast.vec_with_capacity(n)` | `ArenaVec::with_capacity_in(n, ast.allocator())` |
| `ast.alloc(x)` | `ArenaBox::new_in(x, ast.allocator())` |

Imports: `use oxc_allocator::{ArenaBox, ArenaVec};` and `use oxc_str::Str;`
(check which crate name the fork uses — `oxc_allocator::Str` may be re-exported;
prefer whatever compiles with fewest new deps; `Str` lives in `oxc_str`).

Example:

```rust
// OLD
let binding = ast.binding_pattern_binding_identifier(SPAN, ast.str(&name));
// NEW
let binding = BindingPattern::new_binding_identifier(SPAN, &name, ast);
```

Note: `new_*` methods take `impl Into<Ident<'a>>` / `impl Into<Str<'a>>`, so
`ast.str(...)` wrappers are simply dropped — pass the `&str` directly.

## 3. Generated builder methods

Pattern: `ast.<enum_snake_method>(span, args...)` →
`<TargetType>::new_<short_name>(span, args..., ast)`. The `ast` (any
`&impl GetAstBuilder`) moves to the LAST argument; `NONE` arguments are
dropped (the field is `Option`; pass `None` explicitly where the new
signature keeps an `Option` parameter).

Common map (verified against generated/ast_builder.rs):

| OLD deprecated | NEW |
|---|---|
| `ast.expression_identifier(span, name)` | `Expression::new_identifier(span, name, ast)` |
| `ast.identifier_name(span, name)` | `IdentifierName::new(span, name, ast)` |
| `ast.statement_expression(span, expr)` | `Statement::new_expression_statement(span, expr, ast)` |
| `ast.statement_block(span, body)` | `Statement::new_block_statement(span, body, ast)` |
| `ast.expression_call(span, callee, args, optional)` | `Expression::new_call_expression(span, callee, type_args, args, optional, ast)` — NOTE new `type_arguments: Option<...>` param inserted after callee; pass `None` |
| `ast.member_expression_static(span, obj, prop, optional)` | `MemberExpression::new_static_member_expression(span, obj, prop, optional, ast)` |
| `ast.expression_string_literal(span, value, NONE)` | `Expression::new_string_literal(span, value, None, ast)` |
| `ast.expression_object(span, props)` | `Expression::new_object_expression(span, props, ast)` |
| `ast.function_body(span, directives, statements)` | `FunctionBody::new(span, directives, statements, ast)` |
| `ast.formal_parameters(...)` | `FormalParameters::new(..., ast)` |

**Always verify the new signature** in
`$OXC/crates/oxc_ast/src/generated/ast_builder.rs` (or via rust-analyzer) when
migrating a method not in this table. Parameter REORDERING and INSERTED
optional params (e.g. `type_arguments`, `optional`) are common.

`$OXC = /Users/ranger/.cargo/git/checkouts/oxc-2a2560bb11c90978/3f2b4b7`

## 4. AST structural changes (semantic, not mechanical)

### Export declarations split (4 nodes)

- `export <decl>` → `ModuleDeclaration::ExportDeclaration(ExportDeclaration {
  decorators, declaration, .. })`
- `export {a, b}` (local) → `ModuleDeclaration::ExportNamedDeclaration({
  specifiers, export_kind })` — NO declaration/source fields anymore
- `export {a} from 'x'` → `ModuleDeclaration::ExportFromDeclaration({
  specifiers, source: StringLiteral, export_kind, with_clause })`
- `export default ...` → unchanged shape

Every old `export_decl.declaration` / `.source` / `.decorators` access must be
re-dispatched on the new enum variant it actually serves.

### TSModuleDeclaration split

- `module "x" {}` (string id) → `TSExternalModuleDeclaration { id: StringLiteral, body }`
- `namespace X {}` / `module X.Y {}` → `TSNamespaceDeclaration`
- gone from `Statement`/`Declaration` enums under the old name; use the new
  variants.

### Class

- `class.super_class` → `class.heritage: Option<ClassHeritage>`.

### ArrowFunctionBody

Now an enum: `FunctionBody(Box<FunctionBody>) | Expression(...)`. Use
`if let ArrowFunctionBody::FunctionBody(body) = &arrow.body { body.statements }`.

### Function boxing

`MethodDefinition`/`PropertyKey` etc. now store plain `Function<'a>` where
they used to store `Box<'a, Function<'a>>` (and vice versa in places) —
follow the compiler's expected/actual types.

## 5. Verification

After each file: `cargo check -p arkdown-ets-transform 2>&1 | grep '<file>'`.
Migration is complete only when the file has zero errors AND the workspace
builds with zero `deprecated` warnings.

## 6. Additional verified mappings (post-dispatch)

- Arena constructors (`ArenaVec::new_in/with_capacity_in/from_value_in/from_iter_in`,
  `ArenaBox::new_in`) take `&impl GetAllocator` — pass the **AstBuilder reference**
  (`ast`) directly, or `&&Allocator` for a bare allocator. Passing `ast.allocator()`
  (an `&Allocator`) fails: `Allocator: GetAllocator` is not implemented.
- `ast.allocator()` / `ast.builder()` require `use oxc_allocator::GetAllocator;`.
- `Str::from_in(s, alloc)` / `Ident::from_in(s, alloc)` require
  `use oxc_allocator::FromIn;` — use these to arena-allocate names coming from
  short-lived locals (`String::as_str()` etc.), since `From<&'a str> for Str<'a>`
  is borrow-based and `&'static str` needs no allocation.
- `MemberExpression::new_static_member_expression(span, object, property, optional, ast)`
  returns `MemberExpression`, not `Expression` — wrap with `Expression::from(..)`.
- `ArrowFunctionExpression` lost its `expression: bool` field; body shape is
  discriminated by `ArrowFunctionBody::FunctionBody(..) | Expression(..)`.
- Old `alloc_x(...)` builder methods that returned `Box<T>` map to `T::boxed(..., ast)`
  (already returns the ArenaBox — do not double-wrap with `ArenaBox::new_in`).
- `Function::new` still takes `params: ArenaBox<FormalParameters>` — build with
  `FormalParameters::boxed(...)` or wrap `ArenaBox::new_in(FormalParameters::new(...), ast)`.
