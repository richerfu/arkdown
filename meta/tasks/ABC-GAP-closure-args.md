# ABC Compiler Gap: Missing Closure Method Records for Call-Argument Arrow Functions

**From:** analysis agent  
**To:** agent a2ab5563b58d01bf4  
**Date:** 2026-05-28  
**File:** `crates/arkdown-abc/src/ir/builder.rs`

---

## Summary

The **only** missing method records between the golden hvigor `.abc` (9844 bytes) and the
current arkdown `.abc` (4528 bytes) are 3 nested closure records:

- `#~@0>@1*#`
- `#~@0>@1*#^1`
- `#~@0>@1*#^2`

These 3 missing records account for most of the ~5316-byte difference.

---

## Root Cause

These 3 records correspond to the 3 arrow function **arguments** passed to
`observeComponentCreation2(...)` calls inside the `initialRender` method of the `ViewPU`
class:

```typescript
observeComponentCreation2((elmtId, isInitialRender) => { … }, ComponentType)
//                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//                         this arrow is a CALL ARGUMENT, not a variable initializer
```

### Where the gap lives

`collect_arrow_records_from_stmt` (line 702) only handles:

```rust
Statement::VariableDeclaration(decl) => {
    // var f = () => {}  ← handled
}
```

It does **NOT** recurse into `ExpressionStatement` → `CallExpression` arguments to find
arrow functions passed as call arguments. Therefore those 3 arrow bodies never become
`MethodRecord`s in the pre-pass.

### The placeholder bug (line 1500)

Even when `emit_expr` does encounter `Expression::ArrowFunctionExpression`, it emits:

```rust
self.push(Insn::DefineFuncShort(0, 0, length));
//                               ^
//                               always 0 — placeholder, not the real method index
```

No `MethodRecord` is created, and no real method index is computed.

---

## Status

**Implemented** in commit `6c8f3d4` (feat(abc): lower binary/unary/logical expressions and
fix this receiver). The `collect_call_arg_arrow_records` / `collect_call_arg_arrows_stmt` /
`collect_call_arg_arrows_expr` functions handle call-argument arrow collection. The 3
expected closure records (`#~@0>@1*#`, `#~@0>@1*#^1`, `#~@0>@1*#^2`) now appear in the
generated `modules.abc`.

Remaining gap after this fix: 5310 bytes (ours) vs 9844 bytes (golden). The remaining
~4534 bytes are attributable to shorter method body bytecode per method (some expression
kinds still fall to `LdUndefined`) and potentially shorter LNP debug-info records.

---

## Required Fix (original prescription)

### 1. Extend `collect_arrow_records_from_stmt` to handle call arguments

Add a new arm for `Statement::ExpressionStatement` that recurses into
`CallExpression::arguments` and collects any `Expression::ArrowFunctionExpression` found
there (at any depth). Each such arrow becomes a `MethodRecord` with a hvigor-style name
(see naming convention below).

### 2. Emit correct `DefineFuncShort` index in `emit_expr`

In `Expression::ArrowFunctionExpression` (line 1486), look up the pre-compiled
`MethodRecord` by matching the arrow's source span (or by maintaining a counter that
matches the pre-pass order), then use its actual index in:

```rust
self.push(Insn::DefineFuncShort(method_idx, 0, length));
```

### 3. Naming convention

Hvigor names call-argument closures inside a method using ordinal suffixes:

| Occurrence | Name          |
| ---------- | ------------- |
| 1st arrow  | `#~@0>@1*#`   |
| 2nd arrow  | `#~@0>@1*#^1` |
| 3rd arrow  | `#~@0>@1*#^2` |

Pattern: `#~@0>@N*#` for the first (N = 1-indexed method slot), then append `^K` (K
starting at 1) for subsequent arrows in the same method body.

---

## Relevant Code Locations

| File                                   | Lines     | Notes                                                                                                       |
| -------------------------------------- | --------- | ----------------------------------------------------------------------------------------------------------- |
| `crates/arkdown-abc/src/ir/builder.rs` | 679–750   | `collect_class_method_arrow_records` / `collect_arrow_records_from_stmt` — add ExpressionStatement arm here |
| `crates/arkdown-abc/src/ir/builder.rs` | 1486–1501 | `emit_expr` ArrowFunctionExpression arm — fix placeholder index                                             |
| `crates/arkdown-abc/src/ir/builder.rs` | 1502–1506 | `FunctionExpression` arm has same placeholder bug — fix together                                            |

---

## Verification

After the fix, `cargo test -p arkdown-abc` golden-file tests should show the arkdown `.abc`
size and method-record list matching the hvigor reference fixture.
