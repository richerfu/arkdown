# Rolldown Integration Research

This document records the research (conducted 2026-05-28) on whether and how
to integrate the [rolldown](https://github.com/rolldown/rolldown) bundler into
arkdown, and the conclusion reached.

---

## Background

arkdown is a pure-Rust bundler for OpenHarmony OS that reimplements rolldown's
architecture with OHOS-specific semantics (IIFE wrapping, globalThis registry,
OHM URLs, ArkTS support).

The project pins a custom oxc fork:

```toml
oxc = { git = "https://github.com/ohos-rs/oxc",
        rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614",
        features = ["full"] }
```

This fork adds ArkTS (`.ets`) parser support not present in upstream
`oxc-project/oxc`.

---

## OXC Version Constraint (Critical Blocker)

| Parameter | arkdown | rolldown | Compatible? |
|-----------|---------|----------|-------------|
| OXC source | ohos-rs/oxc fork (git) | oxc-project/oxc (crates.io) | ✗ |
| OXC version | 0.129.0 (pinned rev `5caa6d54`) | 0.133.0+ | ✗ |
| ArkTS `.ets` parser | ✓ (custom extension) | ✗ upstream only | ✗ |

The ohos-rs/oxc fork is **4 minor versions behind** upstream. The AST and
parser API changes between 0.129 and 0.133 cause compilation failures when
both are used in the same Cargo workspace.

---

## Option (i): Add rolldown as crate dependency + `[patch]`

### Approach
```toml
# Cargo.toml
[dependencies]
rolldown = { git = "https://github.com/rolldown/rolldown" }

[patch.crates-io]
oxc = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54..." }
oxc_parser = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54..." }
# ... all oxc_* crates
```

### Verdict: ✗ NOT FEASIBLE

**Reason**: rolldown requires oxc 0.133.0 from crates.io. Cargo's
`[patch.crates-io]` redirects crate names but does **not** resolve API
incompatibilities between 0.129.0 and 0.133.0. The workspace would fail to
compile due to missing/changed AST node types.

| Metric | Value |
|--------|-------|
| Feasibility | ✗ Cannot compile |
| Complexity | High |
| Risk | Critical — API incompatibility |

---

## Option (ii): Git submodule + Cargo patches

### Approach
```
# .gitmodules
[submodule "rolldown"]
  path = rolldown
  url = https://github.com/rolldown/rolldown.git

# Root Cargo.toml
[patch.crates-io]
oxc         = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54..." }
oxc_parser  = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54..." }
oxc_ast     = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54..." }
oxc_span    = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54..." }
# ... ~12 more oxc_* crates
```

### Verdict: ⚠️ Technically possible but NOT recommended

| Metric | Value |
|--------|-------|
| Feasibility | Partial — compiles but API breaks at runtime |
| Crates affected | 57 rolldown crates |
| Cargo.toml patches | ~100 lines |
| Maintenance burden | Every rolldown bump needs re-testing |
| Risk | High — AST API differences cause subtle bugs |

**Why not**: rolldown has 57 crates. Patching all their oxc dependencies and
keeping them in sync with the ohos-rs fork revision creates a high ongoing
maintenance burden with no clear benefit.

---

## Option (iii): Fork rolldown into `rolldown/` folder

### Approach
Copy all 57 rolldown crates into the arkdown workspace under a `rolldown/`
directory, update all oxc dependencies to point to ohos-rs/oxc, and adapt
as needed.

```
rolldown/
  crates/
    rolldown/          # Core bundler
    rolldown_common/   # Shared types
    rolldown_plugin/   # Plugin trait
    rolldown_ecmascript/
    ... (53 more crates)
```

### License requirement
rolldown is MIT licensed:
```
Copyright 2024-present VoidZero Inc. & Contributors
```
Every forked file must retain this copyright notice in its header.

### Verdict: ✗ NOT RECOMMENDED (last resort)

| Metric | Value |
|--------|-------|
| Feasibility | Yes — but very high effort |
| Crates to integrate | 57 |
| Files to modify | 150–200+ |
| oxc dependency lines to change | ~100+ across all crates |
| Long-term maintenance | Very high — forking 57 crates freezes rolldown evolution |
| Risk | Very high — diverges from upstream permanently |

---

## Recommended Approach: Reference Architecture, Not Dependency

**Decision: Do NOT integrate rolldown as a dependency or fork.**

### Rationale

1. **oxc version constraint is a hard blocker** — the ohos-rs/oxc fork must
   stay at its current revision for ArkTS syntax support. All three options
   break on this.

2. **OHOS semantics differ from rolldown's target** — IIFE wrapping,
   `globalThis` chunk registry, OHM URLs, and ArkTS transforms are
   OHOS-specific requirements that rolldown does not support and likely
   never will.

3. **Maintenance cost > benefit** — copying 57 crates just to re-implement
   what arkdown already does is wasteful.

4. **Plugin ecosystem already mirrors rolldown** — arkdown's plugin trait
   (`HookUsage` bitflags, async hooks, `register_hook_usage`) was designed
   by studying rolldown. The existing 12+ `arkdown-plugin-*` crates already
   cover all P0/P1 build capabilities.

### What to do instead

| Action | Rationale |
|--------|-----------|
| Use rolldown as design reference | Study its tree-shaking, source maps, and chunk-split algorithms |
| Cherry-pick design patterns | Adapt rolldown's BitSet chunk-split and link-stage design |
| Monitor oxc alignment | When ohos-rs/oxc catches up to upstream 0.133+, reconsider option (ii) |
| Expand arkdown plugins | Fill gaps (ets-transform, TypeScript stripping, import records) |

---

## Plugin Coverage Comparison

| Rolldown built-in plugin | arkdown equivalent | Status |
|--------------------------|-------------------|--------|
| CommonJS transform | rolldown built-in (use it) | — |
| JSON import | `arkdown-plugin-json5` | ✅ Done |
| (none) | `arkdown-plugin-ohos-resolve` | ✅ Done |
| (none) | `arkdown-plugin-module-meta` | ✅ Done |
| (none) | `arkdown-plugin-chunk-wrapper` | ✅ Done |
| (none) | `arkdown-plugin-har-dedup` | ✅ Done |
| (none) | `arkdown-plugin-ets-transform` | ⚠️ Partial |
| Minification | via `oxc_minifier` | 🔲 Not wired |

---

## Oxc Upgrade Path

When the ohos-rs/oxc fork aligns with upstream (currently 4 versions behind):

1. Update `rev` in `Cargo.toml` to the new commit
2. Fix any AST API changes in `arkdown-plugin-ets-transform`
3. Re-evaluate option (ii) — at that point it becomes lower risk

Track progress at: https://github.com/ohos-rs/oxc/commits/main

---

## References

- rolldown repo: https://github.com/rolldown/rolldown
- rolldown Cargo.toml: https://github.com/rolldown/rolldown/blob/main/Cargo.toml
- ohos-rs/oxc fork: https://github.com/ohos-rs/oxc
- rolldown plugin API: `crates/rolldown_plugin/src/plugin.rs`
- arkdown plugin API: `crates/arkdown-core/src/plugin.rs`
