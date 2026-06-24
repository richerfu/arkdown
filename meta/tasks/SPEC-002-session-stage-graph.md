# SPEC-002 Session And Manual Orchestration

## Goal

Replace hvigor task registration and task execution with a typed Rust session
and coarse manual orchestration. The CLI must preserve hvigor's observable
build ordering where required, but it must not introduce a full Rust clone of
hvigor's task runtime, dynamic plugin registry, daemon, or JavaScript lifecycle.

## Hvigor Inputs To Replace

The current hvigor task model is visible in:

- `hvigor/src/base/vigor/task/**`: task API and task dependency graph.
- `hvigor/src/base/vigor/plugin/**`: plugin node descriptors and plugin
  attachment.
- `hvigor/src/base/internal/lifecycle/**`: process/lifecycle hooks.
- `hvigor-ohos-plugin/src/tasks/manager/arkts-task-initializer.ts`: compile
  task creation by module type, language, HAR/HSP mode, preview, test, and
  reload mode.
- `hvigor-ohos-plugin/src/tasks/manager/resource-task-initializer.ts` and
  `native-task-initializer.ts`: resource/native stage creation.
- `hvigor-ohos-plugin/src/tasks/task-creator.ts`: task details, dependency
  declaration, and after-configure behavior.

## Required Rust Model

### Build Session

`BuildSessionBuilder` must construct all stable inputs once:

- project root, selected mode, target, product, signing profile, app config
- module snapshots and target snapshots
- `oh-package.json5` / dependency package snapshots
- SDK tool/schema/compile-plugin metadata
- profile, router map, share config, startup config, privacy/PAC inventory
- resource and source inventories
- native build inventory
- dependency graph snapshot
- input fingerprints
- artifact layout

Later stages must consume `BuildSession`, `ModuleCompileContext`, and typed
plan inputs. They must not re-read manifests or rediscover package roots.

### Build Plan

`BuildPlanBuilder` must convert session snapshots into domain plans:

- `ProfilePlanInput`
- `ResourcePlanInput`
- `BundlePlanInput`
- `BytecodeCompilePlan`
- `PackPlan`
- `SignPlan`
- `NativeBuildPlan`
- `WatchInvalidationPlan`
- `CachePlan`

Domain crates must not depend on `arkdown-session`; plan structs contain the
owned data required by the domain stage.

### Manual Stage Orchestration

`packages/arkdown` must use a small `BuildCoordinator` that executes coarse stages
in explicit order. This is intentionally not a general task runner and not a
dynamic graph API.

| Stage family | Examples | Owner |
| --- | --- | --- |
| validate | project/module/sign/package/pre-check/syscap | `packages/arkdown`, domain validators |
| profile | build profile, module JSON, router/startup/share/privacy/PAC | `arkdown-profile` |
| resource | process resource, compile resource, compiled resource processing | `arkdown-resource` |
| source | source inventory, code-resource copy, visual/worker inventories | `arkdown-session`, `arkdown-resource` |
| bundle | Rolldown bundle and plugin facts | `arkdown` |
| transform | OXC ArkTS transform and checker | `arkdown-ets-transform` |
| bytecode | es2abc / Ark bytecode | `arkdown-ts2abc` |
| native | CMake/Ninja/native libs/strip/binxo | `packages/arkdown::cmd::native_build` |
| package | HAR/HSP/HAP/app package and metadata | `arkdown-pack` |
| sign | validate sign, sign module/app/remote HSP | `arkdown-sign` |
| test modes | unit test, ohosTest, coverage, preview, hot/cold reload | mode inventories plus stage variants |

Stage identity can be represented by fixed enums or explicit methods, but the
initial implementation must favor readable manual sequencing over a complete
`BuildStageGraph`. Hvigor task names can appear only in diagnostics or parity
reports.

## Stage Ordering Contract

Manual orchestration must encode these dependency families:

1. Startup discovery before every stage.
2. Profile/resource preprocessing before bundle.
3. Loader/module metadata generation before Rolldown entry planning.
4. Rolldown bundle before bytecode.
5. Bytecode and compiled resources before packaging.
6. Native library processing before packaging.
7. Package before signing.
8. HAR/HSP dependencies built in dependency waves before dependent modules.
9. Preview/reload/test modes select stage variants, not separate runtimes.

HAR, HSP, HAP, ohosTest, unit test, preview, hot reload, and cold reload must
be explicit orchestration variants built from the same typed plans.

The preferred first implementation shape is:

```text
BuildCoordinator
  -> build session
  -> build domain plans
  -> run profile/resource preparation
  -> run Rolldown bundle
  -> run bytecode/external tools
  -> run pack/sign
```

`BundleStage` must call `Bundler::bundle_with_plan(BundlePlan)`. Rolldown owns
module graph execution, plugin lifecycle, chunk graph generation, render hooks,
and generate-bundle behavior inside that call.

## Custom `hvigorfile.ts` Policy

Arbitrary hvigor JavaScript execution is not allowed in the final production
path. The compatibility model must be:

1. Parse `hvigorfile.ts` / `hvigorconfig.ts` with OXC.
2. Extract a documented declarative subset:
   - build option overlays
   - compile plugin file path declarations that can map to typed transform
     plans
   - known hooks that map to stage extension points
3. Emit typed `BuildScriptSnapshot` and `BuildScriptEffect`.
4. Hard fail with a diagnostic when a script uses arbitrary task registration,
   dynamic code execution, unsupported custom action, or unsupported hook.

The final public API must use generic names such as `BuildScriptSnapshot`,
`BuildScriptEffect`, `StageHook`, and `StageExtension`. Public `Hvigor*`
runtime types must be deleted.

## Parallelism Contract

`BuildCoordinator` must execute modules in dependency waves. Stages inside a
wave may run in parallel only when outputs and cache keys are disjoint.

Parallel jobs must return typed IDs and outputs:

```text
(ModuleId, StageOutput)
(SourceFileId, TransformOutput)
(ResourceEntryId, ResourceOutput)
(PackEntryId, PackEntryMetadata)
```

Final serialization of loader JSON, merged profile artifacts, source maps,
archive entries, signing, and cache database commits must be deterministic.

## Implementation Phases

1. Add `BuildCoordinator` in `packages/arkdown` with explicit coarse stage methods.
2. Keep sequencing manual and readable; do not add a complete DAG until there
   is a measured need for one.
3. Move module selection and dependency wave planning to typed IDs.
4. Convert profile/resource/bundle/bytecode/pack/sign calls to consume domain
   plans from `BuildPlanBuilder`.
5. Replace public `HvigorScript*` and `HvigorTaskGraph*` session types with
   generic build-script and stage-extension snapshots.
6. Add diagnostics for unsupported dynamic `hvigorfile.ts` behavior.
7. Delete hvigor task-name-driven orchestration.

## Verification

- Unit tests for coordinator ordering for HAP, HSP, HAR, bytecode-HAR
  dependency, preview, hot reload, cold reload, unit test, and ohosTest
  variants.
- Snapshot tests proving coarse stage ordering matches the audited hvigor order.
- Read-count tests proving startup inputs are read once per session build.
- `cargo test -p arkdown-session`
- `cargo test -p arkdown-napi -- build`
- `cargo clippy -- -D warnings`
