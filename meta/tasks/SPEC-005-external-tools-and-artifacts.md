# SPEC-005 External Tools And Artifacts

## Goal

Keep true external compiler/packager/signer/native tools as process-backed
stages while replacing hvigor's JavaScript task and command-building layers
with typed Rust plans and runners.

## Allowed External Tool Stages

| Tool family | Examples | Rust owner |
| --- | --- | --- |
| Ark bytecode | `es2abc`, SDK Ark compiler, protoBin/cache-file handling | `packages/arkdown::cmd::build::bytecode` |
| Resources | `restool` and SDK resource compiler/linker binaries | `arkdown-resource` |
| Packaging | SDK package tools or Rust archive writer where selected | `arkdown-pack` |
| Signing | SDK signing/verification binaries | `arkdown-sign` |
| Native build | CMake, Ninja, LLVM strip, binxo, native library checks | `packages/arkdown::cmd::native_build` |
| Device/test | external test/device runners where required | `packages/arkdown` mode runners |

Everything else must move to Rust/OXC/Rolldown.

## Hvigor Inputs To Replace

Audited sources:

- `hvigor-ohos-plugin/src/tasks/worker/sign-job.ts`
- `hvigor-ohos-plugin/src/tasks/sign/**`
- `hvigor-ohos-plugin/src/tasks/compile-resource.ts`
- `hvigor-ohos-plugin/src/tasks/process-resource*.ts`
- `hvigor-ohos-plugin/src/tasks/package-*.ts`
- `hvigor-ohos-plugin/src/tasks/hsp/**`
- `hvigor-ohos-plugin/src/tasks/har/**`
- `hvigor-ohos-plugin/src/tasks/build-native-*.ts`
- `hvigor-ohos-plugin/src/tasks/configure-cmake.ts`
- `hvigor-ohos-plugin/src/tasks/worker/native-strip.ts`
- `hvigor-ohos-plugin/src/tasks/worker/native-binxo.ts`
- `hvigor-ohos-plugin/src/tasks/worker/process-libs.ts`

## Typed Runner Contract

Every external stage must expose:

- typed input plan
- typed command plan
- typed output inventory
- deterministic diagnostics
- cache fingerprint inputs
- artifact paths serialized through `ArtifactPathSerializer` only at final
  artifact boundaries

The command plan must be a result of typed data. It must not be parsed back to
derive build identity.

## Resource Pipeline

`arkdown-resource` must own:

- resource root inventory
- resource filters/excludes
- rawfile and code-resource copy plans
- restool command plan
- compiled resource output inventory
- resource table/index paths
- preview resource variants
- incremental resource invalidation

Resource binary contents must not be stored in `BuildSession`. Writers and
resource compilers stream bytes from paths.

## Profile And Metadata Pipeline

`arkdown-profile` must own:

- app/module profile merge
- generated module JSON
- build profile generation
- router map
- startup config
- share config
- privacy/PAC processing
- permission validation and permission reason processing
- bytecode-HAR metadata inputs

Profile processors consume `ProfileStore`, `ModuleCompileContext` data, and
typed profile plans. They must not perform package discovery.

## Bytecode Pipeline

`packages/arkdown::cmd::build::bytecode` must own:

- SDK es2abc path resolution via plan input
- bytecode command construction
- debug/release flags
- cache-file/protoBin inputs
- bytecode HAR dependency inputs
- ABC output inventory and diagnostics
- optional Rust backend only behind an explicit backend choice

The production backend remains SDK-backed unless a separate Rust backend is
selected and verified.

## Packaging And Signing Pipeline

`arkdown-pack` must own:

- pack entry discovery
- HAR/HSP/HAP/app package layouts
- integrated HSP collection
- bytecode-HAR package metadata
- preload native SO package entries
- archive entry order

`arkdown-sign` must own:

- sign plan validation
- command construction
- signature file outputs
- signed package outputs
- remote HSP signing variants

Archive and signing order must be sequential or have a deterministic final
merge.

## Native Pipeline

`packages/arkdown::cmd::native_build` must own:

- CMake configure plans
- Ninja build plans
- native library collection
- duplicate native library checks
- strip/binxo plans
- debug-symbol collection
- preload SO inventories

Native outputs feed `arkdown-pack` through typed `NativeLibInventory` and
`PackPlan` data.

## Implementation Phases

1. Inventory every external command currently mirrored from hvigor.
2. Define final typed command plans per crate.
3. Move command building out of ad hoc CLI strings into domain runners.
4. Feed command outputs into typed artifact inventories.
5. Wire external stage fingerprints into `arkdown-cache`.
6. Add package/sign/native/resource fixture tests.
7. Delete hvigor command-builder parity shims once equivalent typed runners are
   complete.

## Verification

- `cargo test -p arkdown-resource`
- `cargo test -p arkdown-profile`
- `cargo test -p arkdown-napi --lib bytecode`
- `cargo test -p arkdown-pack`
- `cargo test -p arkdown-sign`
- Fixture builds covering resources, profile merge, bytecode, native SO,
  packaging, and signing.
- Guardrail search:

```bash
rg -n "format!\\(.*\\{.*path|to_string_lossy\\(\\).*cache|replace\\('\\\\\\\\', \"/\"\\)" crates/arkdown-resource crates/arkdown-profile crates/arkdown-pack crates/arkdown-sign packages/arkdown
```
