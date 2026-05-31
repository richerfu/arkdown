# ABC Compiler Gap: Call-Argument Arrow Closure Records

**Date opened**: 2026-05-28
**Last updated**: 2026-05-31
**File**: `crates/arkdown-abc/src/ir/builder.rs`

## Status

Resolved for the minimal-hap fixture.

This note is kept as historical context only. Earlier reports said the current
arkdown ABC was missing three `observeComponentCreation2(...)` call-argument
closure records and was about 5310B vs the 9844B hvigor golden. That is no
longer the current state.

Current minimal-hap state:

- hvigor `ets/modules.abc`: 9844B
- arkdown `ets/modules.abc`: 9700B
- the expected call-argument closure methods are present:
  - `#~@0>@1*#`
  - `#~@0>@1*#^1`
  - `#~@0>@1*#^2`
- all compared method CodeItems are byte-identical to hvigor
- the remaining gap is DebugInfo/LNP/column metadata, not missing closure bytecode

## Original Problem

The original bug was that call-argument arrows were not collected as separate
MethodRecords. In ArkUI output this appears in calls such as:

```typescript
observeComponentCreation2((elmtId, isInitialRender) => { /* ... */ }, ComponentType)
```

The arrow is a call argument, not a variable initializer, so a pre-pass that only
handled `var f = () => {}` missed it.

## Current Guidance

Do not use this note as evidence that method records are still missing. The next
work is tracked in `SPEC-028-abc-structural-parity.md`:

- align source-position events for top-level closures like `Index #*#`;
- align constructor DebugInfo/LNP rows;
- preserve CodeItem parity while converging `ets/modules.abc` bytes.
