# Rolldown Integration Research

This document records research on integrating
[rolldown](https://github.com/rolldown/rolldown) into arkdown.
Updated after the ohos-rs/oxc fork was upgraded to 0.133.0 on 2026-05-28.

---

## Version History

| Date               | oxc (ohos-rs fork)       | rolldown oxc req | Compatible?    |
| ------------------ | ------------------------ | ---------------- | -------------- |
| 2026-05-28 initial | 0.129.0 (`5caa6d54`)     | 0.133.0          | ✗ Blocked      |
| 2026-05-28 revised | **0.133.0** (`c8d02452`) | 0.133.0          | ✅ **Aligned** |

**Build status after upgrade:** 410/410 tests pass. `cargo build --workspace` clean.

---

## Background

arkdown is a pure-Rust bundler for OpenHarmony OS that reimplements
rolldown's architecture with OHOS-specific semantics (IIFE wrapping,
`globalThis` registry, OHM URLs, ArkTS support).

The project pins a custom oxc fork that adds ArkTS (`.ets`) parser support:

```toml
# Cargo.toml — after upgrade to 0.133.0
oxc = { git = "https://github.com/ohos-rs/oxc",
        rev = "c8d02452469e159952f0181b022c1f304e77abee",
        features = ["full"] }
```

---

## Rolldown Workspace Structure (1.0.3)

57 crates in four groups:

| Group                 | Count | Examples                                                                                                                            |
| --------------------- | ----- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Core bundler          | 13    | `rolldown`, `rolldown_common`, `rolldown_ecmascript`, `rolldown_resolver`, `rolldown_plugin`, `rolldown_sourcemap`, `string_wizard` |
| General plugins       | 11    | `asset_module`, `bundle_analyzer`, `replace`, `hmr`, `isolated_declaration`                                                         |
| Vite-specific plugins | 20    | `vite_html`, `vite_css`, `vite_json`, `vite_alias`, …                                                                               |
| Dev / test / NAPI     | 13    | `rolldown_binding`, `rolldown_testing`, `bench`, …                                                                                  |

**Minimum subset for bundler core (no Vite, no NAPI):** ~24 crates.

rolldown's oxc dependencies (all crates.io `0.133.0`):

```toml
oxc             = { version = "0.133.0", features = ["ast_visit", "transformer", …] }
oxc_allocator   = { version = "0.133.0", features = ["pool"] }
oxc_ecmascript  = { version = "0.133.0" }
oxc_traverse    = { version = "0.133.0" }
# … 7 more oxc_* crates
oxc_resolver    = { version = "11.19.1" }  # separate project
oxc_sourcemap   = { version = "6" }        # separate project
oxc_index       = { version = "4" }        # separate project
```

---

## Option (i): Add rolldown as workspace dependency

**Previous verdict (0.129 vs 0.133): ✗ NOT FEASIBLE**
**Updated verdict (both 0.133.0): ✅ NOW FEASIBLE — CURRENT IMPLEMENTATION**

### Implementation

**Root `Cargo.toml` — centralize dependency declarations:**

```toml
[workspace.dependencies]
oxc = { git = "https://github.com/ohos-rs/oxc", rev = "c8d02452469e159952f0181b022c1f304e77abee", features = ["full"] }
oxc_allocator = { git = "https://github.com/ohos-rs/oxc", rev = "c8d02452469e159952f0181b022c1f304e77abee" }
oxc_ast = { git = "https://github.com/ohos-rs/oxc", rev = "c8d02452469e159952f0181b022c1f304e77abee" }
oxc_parser = { git = "https://github.com/ohos-rs/oxc", rev = "c8d02452469e159952f0181b022c1f304e77abee" }
oxc_codegen = { git = "https://github.com/ohos-rs/oxc", rev = "c8d02452469e159952f0181b022c1f304e77abee" }
oxc_transformer = { git = "https://github.com/ohos-rs/oxc", rev = "c8d02452469e159952f0181b022c1f304e77abee" }
rolldown = { version = "1.0.3", default-features = false }
rolldown_plugin = "1.0.3"
rolldown_common = "1.0.3"
```

**`crates/arkdown/Cargo.toml`:**

```toml
[dependencies]
rolldown = { workspace = true }
```

**Key notes:**

- No `[patch.crates-io]` is used. arkdown crates consume the pinned
  `ohos-rs/oxc` fork through `[workspace.dependencies]`; rolldown keeps its own
  upstream OXC dependency graph behind the plugin API boundary.
- All crates under `crates/*` reference internal crates and third-party crates
  with `{ workspace = true }`.
- `default-features = false` excludes NAPI bindings and Vite plugins

### Assessment

| Metric              | Value                                   |
| ------------------- | --------------------------------------- |
| **Feasibility**     | ✅ Viable — versions fully aligned      |
| **Effort**          | Small — ~30 lines Cargo.toml, < 1 day   |
| **Risk**            | Medium-Low                              |
| **Maintenance**     | Low — rolldown updates via `Cargo.lock` |
| **Workspace bloat** | None — rolldown stays external          |

### Likely issues & mitigations

| Issue                   | Probability | Mitigation                                   |
| ----------------------- | ----------- | -------------------------------------------- |
| NAPI linker errors      | Medium      | `default-features = false`                   |
| Transitive dep conflict | Medium      | `cargo tree` + pin conflicting crate         |
| `oxc_resolver` API diff | Low         | Independent project; no ohos-rs patch needed |

---

## Option (ii): Git submodule + Cargo patches

**Verdict: ⚠️ Viable but superseded by Option (i)**

The patch complexity is identical to Option (i) (same 11 redirects), but
adds 57 extra workspace members. Use only if:

- Option (i) hits circular-dependency issues
- You need to patch rolldown's core and contribute upstream

---

## Option (iii): Fork rolldown into `rolldown/` folder

**Verdict: ❌ Last resort only**

Now that Option (i) is viable, forking 57 crates is unnecessary.

If ever needed:

- Copy ~24 core crates (skip Vite plugins, NAPI, bench)
- Redirect `oxc = "0.133.0"` lines to ohos-rs fork
- Add `// Copyright 2024-present VoidZero Inc. & Contributors` (MIT) to every file
- Estimated: 1–2 weeks + ongoing sync cost

---

## Priority Table

| Option                       | Feasibility | Effort          | Risk    | Recommended?   |
| ---------------------------- | ----------- | --------------- | ------- | -------------- |
| **(i) Direct dep + [patch]** | ✅ Yes      | Small (1 day)   | Med-Low | ✅ **DO THIS** |
| **(ii) Submodule**           | ✅ Yes      | Medium (2 days) | Medium  | ⚠️ Fallback    |
| **(iii) Fork 24+ crates**    | ✅ Yes      | Large (1–2 wks) | High    | ❌ Last resort |
| **Reference only**           | ✅ Yes      | None            | None    | ✓ Safe interim |

---

## What rolldown provides (value to arkdown)

| Rolldown component          | Benefit                                                                     |
| --------------------------- | --------------------------------------------------------------------------- |
| `rolldown_sourcemap`        | More mature source map merge/generation                                     |
| `rolldown_resolver`         | Better edge-case coverage (monorepo, pnp, symlinks)                         |
| `rolldown_plugin` framework | Richer async plugin hooks                                                   |
| `string_wizard`             | Magic-string equivalent for source mutations                                |
| Minification wiring         | Production-ready `oxc_minifier` integration (arkdown hasn't wired this yet) |

**OHOS-specific features rolldown does NOT cover (arkdown still owns):**

- IIFE wrapping + `globalThis` registry (`arkdown-plugin-chunk-wrapper`)
- OHM URL resolution (`arkdown-plugin-ohos-resolve`)
- ArkTS transform (`arkdown-plugin-ets-transform`)
- HAR/HSP deduplication (`arkdown-plugin-har-dedup`)
- ABC bytecode compiler (`arkdown-abc`)
- HAP/HSP/HAR packager (`arkdown-pack`)
- DevEco signing (`arkdown-sign`)

---

## oxc Upgrade Path

```
ohos-rs/oxc history:
  rev 5caa6d54  → 0.129.0  (initial; used until 2026-05-28)
  rev c8d02452  → 0.133.0  (current — aligned with rolldown 1.0.3)

rolldown requirement:
  oxc = "0.133.0"  (crates.io, stable)
```

To stay current: monitor ohos-rs/oxc `main` branch; when new commits land,
update `rev` in `Cargo.toml` and run `cargo test --workspace`.

---

## References

- rolldown repo: https://github.com/rolldown/rolldown
- rolldown Cargo.toml: https://github.com/rolldown/rolldown/blob/main/Cargo.toml
- ohos-rs/oxc fork: https://github.com/ohos-rs/oxc (current HEAD = `c8d02452`)
- arkdown plugin API: `crates/arkdown-core/src/plugin.rs`
- hvigorw pipeline: `meta/design/hvigorw-pipeline.md`
