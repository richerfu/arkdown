---
name: arkdown-refactor-plan
description: Mandatory arkdown SPEC-048 architecture refactor contract. Use when Codex edits, reviews, plans, audits, or implements arkdown workspace architecture, crate boundaries, file layout, crate additions/deletions, helper removal, dependency direction, build pipeline flow, plugin layering, or migration phases. Trigger for arkdown tasks mentioning SPEC-048, build session architecture refactor, crate split, crate deletion, utils/helpers cleanup, rolldown alignment, oxc alignment, execution flow, dependency graph, or broad refactor planning.
---

# Arkdown Refactor Plan

## Source Of Truth

Read `meta/tasks/SPEC-048-build-session-architecture-refactor.md` before any
SPEC-048 implementation, review, or planning work. This skill is the mandatory
short-form contract; the SPEC owns the full detail.

## Breaking Rewrite Contract

- Preserve CLI flags and generated artifacts unless SPEC-048 explicitly changes
  them.
- Preserve fixture behavior and hvigor parity expectations.
- Do not preserve old internal helper APIs, old crate exports, or old plugin
  constructors when they block ownership cleanup.
- Do not add thin compatibility wrappers that keep the old architecture alive.
- Delete old helpers in the same change that migrates the last production
  caller.
- Treat minimum viable implementations of the core session model as invalid.

## Final Crate Set

Required additions:

- `arkdown-path`
- `arkdown-ohos-schema`
- `arkdown-session`
- `arkdown-ets-transform`

Required deletions:

- `arkdown-utils`
- `arkdown-loader`
- `arkdown-plugin-router-map`
- `arkdown-plugin-sensitive-check`

Required retained crates keep their narrowed ownership from SPEC-048:

- `arkdown-core`, `arkdown-config`, `arkdown-resolver`, `arkdown`,
  `arkdown-profile`, `arkdown-cache`, `arkdown-abc`, `arkdown-ts2abc`,
  `arkdown-resource`, `arkdown-pack`, `arkdown-sign`, `arkdown-cli`,
  `arkdown-obfuscate`, `arkdown-watcher`, and remaining plugin crates.

## Dependency Direction

- Foundation: `arkdown-core`, `arkdown-path`, `arkdown-ohos-schema`.
- Config/resolver depend on foundation only.
- `arkdown-session` depends on foundation, config, and resolver.
- Domain crates consume typed plans and depend on foundation, not session.
- Plugin crates are thin rolldown adapters and consume session context.
- `arkdown` owns rolldown bundling and must not depend on concrete plugin crates
  or `arkdown-session`.
- `arkdown-cli` is the only crate allowed to compose all stages.
- `arkdown-watcher` consumes session inventories.
- Any dependency edge violating this direction is a refactor blocker.

## File Ownership

- Move path logic to `arkdown-path`.
- Move raw OHOS JSON/JSON5 schemas to `arkdown-ohos-schema`.
- Move startup discovery, snapshots, dependency graph, profile store, SDK
  snapshot, resource inventory, artifact layout, and fingerprints to
  `arkdown-session`.
- Move OXC ArkTS transform core to `arkdown-ets-transform`.
- Keep plugin crates as adapters only.
- Keep profile/resource/pack/sign/cache/abc logic in their owning domain crates
  behind typed plan APIs.
- Keep CLI orchestration in `crates/arkdown-cli/src/cmd/build/`.

## Free Function Rule

Free functions that read files, write files, scan directories, mutate artifacts,
build cache keys, derive stage policy, or own lifecycle must move to an owning
type. Allowed free functions are pure local algorithms in narrow modules.

## Migration Order

Follow the SPEC phase order:

1. Documentation and guardrails.
2. Path foundations.
3. Performance and memory foundations.
4. Build session snapshots.
5. Unified dependency graph.
6. Profile and loader refactor.
7. Resolver and plugin context propagation.
8. CLI pipeline reorganization.
9. Cache and fingerprint hardening.
10. Legacy removal.

## Verification

- Keep generated artifacts byte-for-byte stable where golden tests assert them.
- Run targeted crate tests for moved ownership.
- Use `cargo metadata` to validate acyclic dependency direction after crate
  changes.
- Use `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt` when changes
  cross crate boundaries.
