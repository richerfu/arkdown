# SPEC-004 OXC ArkTS Transform

## Goal

Move all non-external ArkTS/TypeScript/JavaScript compile-plugin behavior into
OXC-backed Rust transforms and checker passes. Raw string rewriting is
forbidden when an OXC AST, visitor, builder, span patch, or codegen path exists.

## Hvigor Inputs To Replace

Audited sources:

- `hvigor-arkts-compose/dist/src/plugins/**`
- `hvigor-arkts-compose/dist/src/project-config/**`
- `hvigor-arkts-compose/dist/src/util/**`
- `hvigor-ohos-plugin/src/tasks/ark-compile.ts`
- `hvigor-ohos-plugin/src/tasks/har/ark-linter.ts`
- `hvigor-ohos-plugin/src/tasks/module-info/ohos-uitransform-optimization.ts`
- `hvigor-ohos-plugin/src/common/obfuscation/**`
- `hvigor-ohos-plugin/src/options/configure/**`

## Required Transform Ownership

`arkdown-ets-transform` must own:

- ArkTS parser configuration and syntax-mode selection.
- Component/decorator transforms.
- Builder-style and ViewPU lowering.
- API transform/version gating.
- Resource/syscap rewrite.
- Lazy import and kit import rewrite.
- Sendable and semantic checks.
- Router/share/startup/profile-related source facts.
- Import/export analysis required by wrapper chunk behavior.
- Obfuscation input facts and name preservation facts.
- Source-map emission.
- Checker diagnostics replacing hvigor's ArkTS lint/check behavior.

Transform callers must pass `TransformPlan` or narrower typed inputs. The
transform crate must not read project manifests, package manifests, profile
files, SDK metadata, or dependency roots.

## OXC Data Lifetime

Each transform call owns its OXC allocator. The only data allowed to escape the
call is owned:

- emitted code
- source map
- diagnostics
- span ranges
- typed facts
- stable module metadata
- content fingerprints

OXC allocator-backed AST references must not be stored in `BuildSession`,
caches, plugin structs, or long-lived stage state.

## Custom Compile Plugin Policy

Hvigor allows `arkOptions.compilePluginFile` and custom compile plugin behavior.
The replacement policy is:

1. Detect plugin paths through the latest hvigor configuration/runtime surface.
2. Load TypeScript/JavaScript in the Node package's existing V8 process and
   apply the latest-hvigor export, validation, diagnostics, and ordering rules.
3. Wrap supported standard hooks as a real `rolldown_plugin::Plugin` through
   the same-process NAPI boundary; Rolldown owns their execution.
4. Add only thin typed adapters for observed latest-hvigor hooks absent from
   Rolldown (`beforeBuildEnd` and `shouldInvalidCache`).

Arkdown-owned transforms remain native OXC/Rust passes. The JS path exists only
for arbitrary user plugin code that hvigor itself executes; declarative OXC
approximation, subprocess execution, and a second plugin scheduler are
forbidden.

## Source Rewrite Policy

Allowed rewrite mechanisms:

- OXC AST mutation and codegen.
- OXC visitor plus builder APIs.
- OXC span collection plus deterministic span patch application.
- Structured serializers for JSON/profile/module metadata.

Forbidden rewrite mechanisms:

- ad hoc line-based string edits.
- regex source mutations for syntax-aware transforms.
- manual JavaScript string assembly for generated modules when AST/codegen is
  available.
- nondeterministic patch ordering.

## Diagnostic Policy

Diagnostics must include:

- source file identity
- span where available
- stage and transform pass name
- target API / build mode when relevant
- deterministic code or category

The checker must not depend on hvigor logger adapters. CLI converts typed
diagnostics to user-visible output.

## Implementation Phases

1. Define final `TransformPlan`, `TransformInput`, `TransformOutput`, and
   transform fact types.
2. Move all remaining transform logic out of old plugin crates into
   `arkdown-ets-transform`.
3. Replace source rewrites with OXC AST/codegen or deterministic span patches.
4. Port wrapper-chunk import/export analysis to OXC.
5. Port router/share/startup/resource/syscap facts to typed transform outputs.
6. Replace hvigor ArkTS lint/check behavior with OXC checker passes.
7. Add unsupported custom compile plugin diagnostics.
8. Delete legacy plugin transform implementations and hvigor-specific format
   shims when final callers are migrated.

## Verification

- `cargo test -p arkdown-ets-transform`
- Focused transform fixtures for decorators, component lowering, ViewPU,
  resource/syscap rewrite, lazy import, kit import, API transform, sendable,
  source maps, router map, and share/startup metadata.
- Golden tests proving generated code/source maps are stable.
- Guardrail search:

```bash
rg -n "replace\\(|regex|MagicString|compilePluginFile|hvigor_format" crates/arkdown-ets-transform crates/arkdown-plugin-ets-transform
```

Any remaining raw rewrite must have a local comment explaining why no OXC path
exists and must apply deterministic span patches.
`crates/arkdown-plugin-ets-transform` is a migration source only and must be
deleted after its final adapter moves into `arkdown`.
