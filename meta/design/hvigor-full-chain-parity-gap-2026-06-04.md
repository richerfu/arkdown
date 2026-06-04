# hvigor Full-Chain Parity Gap Analysis

Date: 2026-06-04

Baseline:

- Local arkdown branch: `fix/abc-alignment-upstream`
- Local hvigor upstream reference: DevEco Studio `6.0.0.858` under
  `/Users/ranger/.meat/ide/DevEco-Studio-6.0.0.858.app/Contents/tools/hvigor/hvigor-ohos-plugin/src/tasks/`
- Main upstream tasks checked: `merge-profile.js`, `process-profile.js`,
  `process-share-config.js`, `process-startup-config.js`,
  `generate-loader-json.js`, `compile-resource.js`,
  `generate-pkg-module-json.js`, `process-module-privacy-profile.js`

## Conclusion

The local implementation is not fully aligned with hvigor yet. Some narrow
fixtures, especially `minimal-hap`, are close or already byte-compatible in
selected artifacts, but the full build chain still contains structural gaps.

The biggest blockers for "compile exactly like hvigor" are:

1. Profile is reconstructed from a Rust whitelist, while hvigor preserves and
   mutates the merged JSON object. Unsupported fields are silently dropped.
2. Share/startup/privacy profile tasks are not fully wired into the build and
   resource phases.
3. Loader dependency maps are structurally present but many maps are still empty
   or only partially populated.
4. CompileArkTS handoff is not byte-stable for non-trivial projects because
   Rollup module order, ViewPU transforms, dependency graph data, and source map
   details still diverge.
5. Release obfuscation is currently applied to temporary JS chunks, but final
   ABC compilation uses CompileArkTS cache TS inputs, so the obfuscation step
   does not reliably affect final ABC bytes.
6. The Rust ABC backend has improved, but it is still a custom compiler with
   missing upstream es2abc cache/protoBin/release optimization/non-merge paths
   and unsupported syntax cases.
7. Packaging has partial HAP parity, but HSP/HAR, integrated HSP, preload SO,
   privacy profile, ByteCode HAR runtime packaging, and deterministic signing
   are not fully aligned.

Full parity can only be claimed after an oracle harness compares hvigor and
arkdown from clean builds across the fixture matrix listed in the repair plan.

## Parity Target

For each supported project shape and build mode, parity means:

- Same accepted/rejected inputs and same severity for validation errors.
- Same intermediates where hvigor writes an artifact:
  `merge_profile`, `process_profile`, `res`, `loader`, `loader_out`,
  `source_map`, `importee`, `package`, `hap_metadata`, cache metadata.
- Same final unsigned package bytes for deterministic package stages.
- Same final signed package semantics. Byte-identical signatures may require
  controlling timestamps, signing block metadata, and certificate inputs.
- Same runtime-relevant loader/profile/resource/ABC behavior for non-byte-stable
  stages.

## Full-Chain Difference Matrix

### 1. Build Configuration And Target Model

Upstream behavior:

- hvigor starts from project/module `build-profile.json5`, product target data,
  `hvigor-config.json5`, CLI properties, source-set overrides, custom resources,
  and `hvigorfile.ts` plugin logic.
- `MergeProfile` consumes product options such as `removePermissions`,
  `integratedHsp`, `resOptions.includeAppScopeRes`, target-specific ability
  overrides, sanitizer flags, native options, and bundle type behavior.
- `GenerateLoaderJson` consumes `arkOptions.runtimeOnly`, `byteCodeHar`,
  `bundledDependencies`, `aotCompileMode`, `apPath`, `sourceOption.workers`,
  `strictMode.useNormalizedOHMUrl`, and dependency-manager results.

Local behavior:

- `arkdown-config` parses a smaller subset of `build-profile.json5`.
- The build CLI has only `--target`, `--mode`, `--module`, `--product`, and
  `--project`.
- `BuildOption` currently models strict mode, partial ark options,
  `resOptions`, `sourceOption`, and `preloadSo`, but many hvigor build options
  are not represented or not wired.

Confirmed gaps:

| Area | Current status | Risk |
| --- | --- | --- |
| `hvigorfile.ts` custom logic | Not executed | User/plugin mutations never happen |
| Source-set/target overlays | Only partially represented | Wrong module/profile/resource inputs |
| `buildModeSet`, `targets.source.abilities` | Not fully applied | Ability icon/label/launchType overrides diverge |
| `removePermissions` | Not modeled in profile merge | Permission output diverges |
| `arkOptions.integratedHsp` | Pack support exists in crate, CLI passes empty list | Integrated HSP package misses files |
| `arkOptions.byteCodeHar` / bundled deps | Partial scan only | HAR/HSP compile/package mode diverges |
| `sourceOption.workers` | Not used by loader generation | Worker compile entries missing |
| `nativeLib`, CMake/native build | Not implemented as hvigor tasks | Native artifacts/preload metadata diverge |
| AOT fields `aotCompileMode`, `apPath`, `hostPGO` | Loader emits fixed/default fields | `anBuildOutPut`, `anBuildMode`, `apPath` differ |

### 2. Profile Merge, Process, And `module.json`

Upstream behavior:

- `MergeProfile` deep-clones app/module JSON, merges product and dependency
  data, then writes a full JSON object.
- `ProcessProfile` reads the merged object and mutates it in place:
  `virtualMachine`, `compileMode`, `deviceTypes`, `dependencies`, HAR ability
  collection, fake preview ability, form filtering, and duplicate ability checks.
- It writes the whole object. Fields not touched by the task are preserved.

Local behavior:

- `ModuleJson5Module`, `HapManifest`, `AbilityEntry`,
  `ExtensionAbilityEntry`, and `module_json_writer` form a whitelist pipeline.
- `process_profile_with_ark_vm_version()` maps selected fields into
  `HapManifest` and drops anything not modeled.
- `module_json_writer::manifest_to_json()` reconstructs `module` from scratch.

Key code paths:

- `crates/arkdown-core/src/manifest.rs`
- `crates/arkdown-profile/src/process_profile.rs`
- `crates/arkdown-profile/src/module_json_writer.rs`
- `crates/arkdown-cli/src/cmd/build.rs`

Confirmed field-level gaps:

| Field group | hvigor behavior | Local status | Impact |
| --- | --- | --- | --- |
| `app` unknown/schema fields | Preserved after merge unless explicitly changed | Hardcoded output list | Any app field outside the list is dropped |
| App sanitizer flags | `asanEnabled`, `tsanEnabled`, `hwasanEnabled`, `ubsanEnabled` may be added | Not modeled | Runtime/debug package metadata differs |
| App `minCompatibleVersionCode`, `multiProjects`, extra schema fields | Preserved/merged by hvigor | Not generally preserved | `module.json` mismatch |
| `module.packageName` | Set from `oh-package.json5` package name | Set to `manifest.module_name` | HAP/HAR/HSP metadata can be wrong |
| `module.dependencies` | Built from HSP dependency info | Always empty in writer | HSP dependency metadata missing |
| `deliveryWithInstall`, `installationFree` | Derived from bundle type/atomic service | Hardcoded `true`/`false` | Atomic service and feature delivery differ |
| `atomicService` | Customized by bundle type/target config | Not modeled | Atomic service package diverges |
| `requestPermissions` | Merged from module and HAR deps, can remove configured permissions | Dropped | Permission declarations missing |
| `definePermissions` | Preserved by full-object processing | Dropped | Permission provider metadata missing |
| `module.metadata` | Merged from HAR deps and resource tasks | Dropped in final writer | Router/shortcut/distro/custom metadata can disappear |
| `proxyData` | Preserved and bundleName-adjusted | Dropped | Proxy data runtime config missing |
| `compressNativeLibs` | Can be added from hvigor config | Not modeled | Native packaging metadata mismatch |
| `appStartup` | Preserved or added by resource/package tasks | Not written from manifest | Startup profile can be disconnected |
| `crossAppSharedConfig` | Usually `$profile:<name>` after resource phase | Current type is `Vec`, not `$profile` string | Source profile can fail to parse or final reference is missing |
| Ability extra fields | Preserved by upstream object mutation | Only name/srcEntry/exported/skills/icon/label/description/start window fields | Fields such as launch/display/orientation/background modes are dropped |
| Ability `metadata` | Used for shortcuts and other ability metadata | Not modeled for normal abilities | Shortcuts and metadata resources missing |
| Extension ability extra fields | Preserved | Only name/srcEntry/type/label/description/metadata | Extension-specific fields can be dropped |
| Metadata entry extra fields | Preserved | Only `name`, `value`, `resource` | Metadata with additional schema fields can be truncated |
| Validation severity | hvigor schema/duplicate checks may stop build | Several local checks warn or skip | Invalid projects may build with wrong output |

Important structural issue:

- `module.crossAppSharedConfig` in hvigor can be a `$profile:<name>` string
  used to locate `resources/base/profile/<name>.json`.
- Local `ModuleJson5Module.cross_app_shared_config` is typed as
  `Vec<CrossAppSharedConfigEntry>`. A real string value is not equivalent and
  may fail deserialization instead of driving the share-config task.

### 3. Share Config, Startup Config, Privacy Profile

Upstream behavior:

- `ProcessShareConfig` reads module/dependency share config, validates duplicate
  URI conflicts, writes `temp-share-config.json`.
- `CompileResource.processShareConfig()` writes
  `resources/base/profile/<share>.json` and updates `module.json` to
  `crossAppSharedConfig: "$profile:<share>"`.
- `ProcessStartupConfig` validates schema and paths, fills `moduleName`,
  `startupConfigPath`, `srcEntryAbsolutePath`, and `ohmurl`, merges local and
  remote HAR startup config, handles preload SO startup tasks, and errors on
  duplicate names.
- `CompileResource.processStartupConfig()` strips intermediate-only fields,
  writes the merged startup profile under resources, and adds `module.appStartup`
  when needed.
- `ProcessModulePrivacyProfile` validates/copies PAC/privacy profile artifacts.

Local behavior:

- `arkdown-profile` has `merge_share_config()` and `merge_startup_config()`.
- The CLI build path does not call them in the normal profile/resource flow.
- `write_static_intermediate_files()` writes an empty
  `temp-share-config.json`.
- No resource-phase rewrite integrates share/startup profile JSON back into
  `res/default/resources/base/profile`.
- CLI passes `privacy_profile_path` into `PackInput`, but `arkdown-pack` does
  not currently package it as `module.privacy.json`.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| Share config invocation | Functions exist but build path does not call them | Real `crossAppSharedConfig` is not compiled |
| Share config profile reference | No resource rewrite to `$profile:<name>` | Final `module.json` misses runtime profile link |
| Share duplicate URI | Local warning; hvigor fatal | Invalid output can be produced |
| Startup invocation | Functions exist but build path does not call them | Startup tasks missing from compile entries/resources |
| Startup `ohmurl` / absolute src entry | Local merge does not fully populate like hvigor | Compile entries and runtime startup metadata diverge |
| Startup duplicate task names | Local warning; hvigor fatal | Invalid task graph can pass |
| Startup dependency validation | Not equivalent to hvigor resource-phase validation | Broken startup dependencies may ship |
| Preload SO startup tasks | Not implemented | `systemPreloadHintStartupTasks` missing |
| Privacy/PAC | Field exists; packager ignores it | `module.privacy.json` missing |

### 4. Resource Compilation And Resource-Driven Profile Rewrites

Upstream behavior:

- Resource compilation takes `process_profile/default/module.json` and resource
  config files, compiles resources, then runs custom post-processing:
  router map, share config, startup config, form extension abilities, distro
  filter, shortcuts, request-permission reason resource extraction, preloaded SO
  handling.

Local behavior:

- `arkdown-resource` compiles resources and can produce IDs used by
  `module_json_writer`.
- `write_resource_config_files()` writes a minimal `resConfig.json`.
- Router map has partial special handling.
- Share/startup/privacy/native post-processing is missing or partial.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| Resource config dependencies | `dependencies` is `[]` | HSP/HAR resource tables not consumed |
| HSP resources map | `loader_json.hsp_resources_map` defaults empty | HSP resource lookup differs |
| `resource.directories` / target custom resources | Not fully wired | Wrong resource set for target builds |
| Permission reason resource extraction | Not implemented | `resource_str` outputs differ |
| Distro filter metadata/profile | Not implemented | Distribution filtering differs |
| Shortcuts profile handling | Not implemented | Ability shortcuts metadata/resources missing |
| Form extension resource validation | Partial widget handling only | Form module resource behavior differs |
| Preloaded SO resource task | Not implemented | Native preload metadata missing |

### 5. `loader.json`

Upstream behavior:

- `GenerateLoaderJson` builds loader metadata from dependency-manager state,
  package context info, router/share/startup/insight-intent inputs,
  runtime-only/dynamic import config, ByteCode HAR metadata, workers, HSP/HAR
  OHM maps, AOT fields, update-version info, and package manager mode.

Local behavior:

- `LoaderJson` contains many hvigor top-level fields.
- Several fields are generated for minimal projects, but important dependency
  maps are defaulted or only partially populated.

Confirmed gaps:

| Loader field | Current status | Impact |
| --- | --- | --- |
| `modulePathMap` | Workspace modules only | Dependency module paths incomplete |
| `byteCodeHarInfo` | Direct scan of `oh_modules`; partial metadata | Full dependency-manager parity missing |
| `declarationEntry` | Only copied from detected ByteCode HAR metadata | Missing generated entries from ability/startup/runtime-only/workers |
| `hspNameOhmMap` | Always empty in generator | HSP import/runtime resolution differs |
| `harNameOhmMap` | Always empty in generator | HAR dynamic/import metadata differs |
| `hspResourcesMap` | Always empty | HSP resources not mapped |
| `compileEntry` | Runtime-only/router/worker partial | Missing dependency ability entries, startup entries, insight intent, sourceOption workers, bundled deps |
| `otherCompileFiles` | Always empty | ByteCode HAR other compile entrances missing |
| `dynamicImportLibInfo` | Product/module direct cases partial | Transitive runtime-only and dependency-host info incomplete |
| `routerMap` | Partial | Resource/task coupling incomplete |
| `updateVersionInfo` | Always empty | ByteCode HAR dependency version update metadata missing |
| `workers` | Scans `src/main/ets/workers` only | buildOption/package metadata workers missing |
| `anBuildOutPut`, `anBuildMode`, `apPath`, `fallbackAnBuild` | Fixed/default | AOT behavior differs |
| `patchConfig`, `buildConfigPath` | Not equivalent | Preview/test/patch builds diverge |
| `supportChunks` | Internal field fixed false | manualChunks path is effectively disabled by default |

### 6. Dependency Resolution And OHM URL Generation

Upstream behavior:

- hvigor dependency-manager classifies local/remote HAR, HSP, ByteCode HAR,
  SO, dynamic dependencies, bundled dependencies, and transitive dependencies.
- OHM URLs are generated from package context info, normalized URL mode, bundle
  name, module name, dependency names, and source roots.

Local behavior:

- Resolver is OXC/Rolldown based with `oh_modules` roots and SDK API dirs.
- It resolves many normal imports, but does not have full hvigor dependency
  manager state.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| Dependency type classification | Partial | Wrong HAR/HSP/SO/ByteCode behavior |
| Remote HSP / integrated HSP graph | Not wired through CLI | Missing maps and packaged HSPs |
| Transitive runtime-only/dynamic imports | Partial | Missing compile entries and loader info |
| `useNormalizedOHMUrl=false` behavior | Not fully proven | Old OHM URL paths can diverge |
| Module order | Rolldown/DashMap order can differ from Rollup Map order | `filesInfo.txt`, `modules.cache`, ABC record order can diverge |

### 7. ArkTS Transform And CompileArkTS Handoff

Upstream behavior:

- hvigor uses ets-loader/Rollup plugins and the ArkTS transform pipeline. The
  CompileArkTS handoff is based on Rollup's module graph and deterministic
  insertion order.

Local behavior:

- `arkdown-plugin-ets-transform` and OXC/Rolldown implement a native pipeline.
- Handoff files are written, and minimal cache TS parity exists for simple
  fixtures.
- Larger ViewPU/ArkUI projects still require byte-level comparison against
  hvigor.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| ViewPU transform parity | Many cases implemented, but not proven complete | Generated TS differs before ABC |
| Unsupported/special AST constructs | ABC builder still rejects or skips some constructs | Compile failure or wrong bytecode |
| Full TypeScript semantic behavior | No full tsc/hvigor checker equivalence | Transform and diagnostics differ |
| Rollup plugin parity | Native plugins are substitutes, not exact ports | Handoff and side effects can differ |
| Source maps | HAP minimal path works, broad parity unproven | Debug LNP and package maps differ |
| Handoff order | Known order risk from Rolldown module storage | ABC record/string/table order differs |

### 8. Bundling, Tree Shaking, Chunks, And Release Obfuscation

Upstream behavior:

- Debug disables treeshake.
- Release enables treeshake and may use manual chunks when `supportChunks` is
  enabled.
- Obfuscation/minification runs in the release pipeline before final ABC input.

Local behavior:

- Bundler sets `treeshake=false` for debug and `true` for release.
- Manual code splitting exists, but only when `loader_json.support_chunks` is
  true. That field is currently generated as false.
- Obfuscation is wired in CLI, but it mutates `bundle_out.chunk_files`. Final
  ABC compilation uses `compile_arkts_handoff.inputs`, so obfuscation may not
  affect the files passed to `compile_to_abc`.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| Release treeshake | Enabled in bundler | Needs fixture parity vs hvigor |
| `supportChunks` | Code path exists; config always false | Multi-chunk output disabled |
| Obfuscation target files | Runs on temp JS chunks, not guaranteed ABC inputs | Release ABC can be unobfuscated/different |
| Obfuscation rule parity | Pure Rust subset | SDK obfuscation behavior not equivalent |
| Terser/minifier parity | Not equivalent to hvigor plugin stack | Release JS/ABC input differs |
| Cache invalidation by obfuscation | Task cache has partial hash | Rollup/ABC cache invalidation not equivalent |

### 9. ABC / ts2abc / es2abc Compatibility

Upstream behavior:

- hvigor invokes es2abc/ark frontend with exact flags, module mode, package
  entry list, cache/protoBin behavior, target API, debug info, source positions,
  and release optimizations.

Local behavior:

- `arkdown-ts2abc` accepts more upstream flags than before.
- `arkdown-abc` is a Rust ABC compiler that emits valid Panda ABC for many
  cases and has minimal fixture parity improvements.
- It is still not a full es2abc replacement.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| es2abc cache format | Custom JSON cache, not upstream msgpack/protoBin | Incremental/build cache parity missing |
| `merge_abc=false` | Still not implemented | Non-merged ABC mode unsupported |
| Release optimization | `opt_level=2` warns/partial, not full optimizer | Release bytecode differs |
| ProgramCache/protoBin | Not implemented | Incremental and DevEco compatibility missing |
| Unsupported AST/syntax | Some constructs error or are skipped | Broad project compile parity missing |
| Computed auto-accessor | Still not aligned | Incorrect class/accessor output possible |
| Debug info/LNP | Improved, but only fixture-proven | Debugger/source location can differ |
| ABC reader/validation | Minimal validation | Corrupt/incompatible ABC may pass checks |
| Upstream tests | Smoke/opcode tests, not byte-level golden suite | Regressions can pass CI |

### 10. ByteCode HAR

Upstream behavior:

- ByteCode HAR metadata is represented in `loader.json`.
- ByteCode HAR ABC files are not raw-appended to the consumer `modules.abc`.
- Declaration entries, runtime-only entries, update-version info, insight intent,
  and package context are derived from dependency-manager data.

Local behavior:

- Previous raw byte append was removed.
- `byteCodeHarInfo` is populated from a direct scan.
- Builds with ByteCode HAR dependencies emit warnings.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| Metadata scan | Direct filesystem scan | Does not match hvigor dependency graph |
| Declaration entry generation | Partial copy only | Loader metadata incomplete |
| Runtime package path | Not proven end-to-end | Device/runtime loading may fail |
| `updateVersionInfo` | Empty | Version mismatch handling missing |
| Insight intent/runtime-only/workers | Partial or missing | Compile/load metadata differs |

### 11. Widget/Form Compilation

Upstream behavior:

- ArkTS widgets/forms use a separate widget loader JSON and separate widget
  compile/cache path.

Local behavior:

- `compile_widget_pass()` exists and writes widget handoff data.
- It uses a separate CompileArkTS cache root for widget TS, but ABC cache is
  still `build/default/cache/abc`.
- It calls the normal `compile_to_abc()` path rather than the higher-level
  `arkdown-abc::compile_widget()` wrapper.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| Widget cache semantics | Partially separate | Cache collisions/mismatch possible |
| Widget loader parity | Writes `widget_loader.json`, but not fully hvigor-proven | Widget compile metadata can differ |
| Widget ABC bytes | Known mismatch in larger projects from previous reports | `widgets.abc` not equivalent |
| Form validation/resource path | Partial | Form package behavior differs |

### 12. Packaging, Signing, Native, HSP/HAR

Upstream behavior:

- HAP/HSP/HAR packaging depends on processed module JSON, resources, ABC,
  native libs, integrated HSPs, privacy/PAC, preload SO, source maps, pack info,
  compression settings, archive order, and signing.

Local behavior:

- HAP packer can produce minimal packages.
- HSP reuses HAP layout.
- HAR source packaging is simpler than hvigor's full HAR/ByteCode HAR flows.
- Native libs are collected heuristically from `src/main/libs`, `libs`,
  `oh_modules`, and project `oh_modules`.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| HSP packaging | Reuses HAP pack; limited dependency/resource maps | HSP output not proven |
| HAR source/ByteCode modes | Basic source HAR only | HAR artifact parity missing |
| Integrated HSP | Pack crate supports entries; CLI passes empty vec | Integrated remote HSPs missing |
| Preload SO | Ignored tests and no full task flow | Preload metadata/native package differs |
| Privacy/PAC | Pack input exists; archive write missing | `module.privacy.json` missing |
| Native build/CMake | Not implemented | Native outputs missing |
| Pack info | Uses fixed/default values in multiple places | Package metadata differs |
| Signing | Implemented sign-or-skip | Byte-identical signing not guaranteed |

### 13. Incremental Cache And Build Skipping

Upstream behavior:

- hvigor uses task input declarations, Rollup msgpack cache, invalid-cache
  plugin behavior, obfuscation hash invalidation, module JSON/SDK cache cleanup,
  and es2abc cache/protoBin.

Local behavior:

- CLI has `TaskInputs` for coarse compile skipping.
- `arkdown-cache::CompileEnv` is used in that coarse key.
- ABC has a custom cache path/file contract.
- Rollup/Rolldown cache parity and es2abc cache parity are not implemented.

Confirmed gaps:

| Area | Current status | Impact |
| --- | --- | --- |
| Task-level cache | Partial coarse cache | Can skip incorrectly or miss cache hits |
| Rollup msgpack cache | Not wired | Incremental behavior differs |
| Invalid-cache plugin | Partial/router-specific only | Stale generated files possible |
| es2abc `--cache-file` | Custom cache | DevEco/upstream cache compatibility missing |
| Cache cleanup on module/app changes | Partial | Stale ABC/TS outputs possible |

## Local Custom Logic That Must Be Reconciled

These are intentionally custom or substitute implementations. They can remain
only if oracle tests prove output equivalence.

| Component | Local custom logic | Required action |
| --- | --- | --- |
| Profile writer | Whitelist reconstruction | Replace with preserve-and-mutate JSON or prove every field |
| Resource compiler | Rust restool substitute | Add broad golden tests |
| Resolver | OXC/Rolldown resolver plus custom OHOS plugin | Compare against hvigor dependency manager outputs |
| Bundler | Rolldown instead of Rollup | Stabilize order and chunk metadata |
| ETS transform | Native transform implementation | Golden compare cache TS and source maps |
| Obfuscation | Pure Rust obfuscator | Apply to real ABC inputs and compare release outputs |
| ABC compiler | Rust es2abc substitute | Expand byte-level golden test matrix |
| Packager | Rust zip/package writer | Compare zip entry order/method/content and metadata |
| Validation | Warnings in several places where hvigor exits | Align error severity |

## Gap-By-Gap Upstream Alignment Checklist

This section maps each major difference to the concrete upstream behavior that
must be mirrored locally. It is the implementation checklist for closing the
matrix above.

### A. Build Config And Target Data

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Missing `hvigorfile.ts` effects | hvigor executes project/module plugins before task config is finalized | Add an explicit extension surface or a compatibility loader for the subset we need; at minimum, capture generated target/profile/build options from hvigor oracle and model those fields in Rust | Fixtures that mutate target/profile data through hvigorfile produce the same effective config |
| Partial target/source-set overlays | `MergeProfile` uses target source-set model and build-profile target options | Extend `arkdown-config` to resolve project/module/product/target/source-set layers into one `EffectiveTargetConfig` | Effective config JSON from arkdown matches hvigor target data dump |
| Missing ability overrides from build-profile | `MergeProfile.mergeBuildProfileAbilities()` | Parse `targets[].source.abilities[]`; apply icon/label/launchType by ability name before writing profile | Ability fields in `merge_profile` and final `module.json` match hvigor |
| Missing `removePermissions` | `MergeProfile.getRemovePermissionSet()` and request permission merge | Parse `buildOption.removePermissions`; merge HAR permissions first, then remove only dependency permissions as hvigor does | Permission-heavy HAP fixture matches hvigor |
| Missing integrated HSP CLI wiring | `ProcessIntegratedHsp`, pack `libs/<hsp_dir>/<hsp>` | Resolve remote HSP dependencies marked integrated; call `normalize_integrated_hsps`; pass entries into `PackInput.integrated_hsps` | Package contains the same embedded HSP files and patched pack.info |
| Missing source workers | `TargetTaskService.getWorkerConfig()` and `GenerateLoaderJson.getWorkerConfig()` | Parse `sourceOption.workers`; convert to absolute paths; merge with dependency metadata workers | `loader.json.workers` and CompileArkTS entries match hvigor |
| Missing AOT fields | `GenerateLoaderJson.handleAotFields()` | Parse `aotCompileMode`, `apPath`, `hostPGO`; copy `.ap` to intermediate path and emit `anBuildOutPut`, `anBuildMode`, `apPath`, `fallbackAnBuild` exactly | AOT fixture loader JSON matches hvigor |

### B. Profile / `module.json`

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Whitelist profile reconstruction drops fields | `MergeProfile` + `ProcessProfile` mutate a full JSON object | Replace `HapManifest`-only writer with a preserve-and-mutate `ProfileDocument` backed by `serde_json::Value` or ordered maps; typed structs become views, not storage | Unknown schema-valid fields survive all profile stages |
| Incorrect `packageName` | `MergeProfile` sets `module.packageName = packageJsonObj.name` | Read `oh-package.json5#name`; write it as `module.packageName`; keep module name separate | HAP/HAR/HSP packageName equals hvigor |
| Empty `dependencies` | `ProcessProfile.addModuleDependencies()` | Build HSP dependency list with bundleName/moduleName/versionCode rules; obey `ohos.har.excludeHspDependencies` | HSP dependency fixture matches hvigor |
| Hardcoded `deliveryWithInstall` / `installationFree` | `MergeProfile.customizeModuleOpt()` | Derive from bundle type, atomic service, and target config; do not hardcode | Atomic service and feature module fixtures match hvigor |
| Dropped permissions/metadata/proxyData/definePermissions | Full-object profile mutation preserves these | Preserve all unknown fields; implement specific mutations only where hvigor mutates | Final `module.json` field set matches hvigor |
| Dropped normal ability metadata and extra fields | Upstream keeps ability objects and mutates selected fields | Preserve full ability objects; overlay resource IDs and build-profile ability overrides without rebuilding object | Ability-rich fixture matches hvigor |
| Dropped extension ability extra fields | Upstream keeps extension ability objects | Preserve extension objects; only normalize `type`, metadata resource IDs, and dependency additions | Extension ability fixture matches hvigor |
| `crossAppSharedConfig` typed incorrectly | hvigor treats module value as `$profile:<name>` reference | Change input model to `Option<ProfileRefOrInlineShareConfig>`; support `$profile` string and inline array if needed; final resource stage writes `$profile:<name>` | Share config fixture parses and final module JSON matches |

### C. Share / Startup / Privacy Tasks

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Share config not called | `ProcessShareConfig.doTaskAction()` | Add a build step before resource compilation: collect self/dependency share configs and write `intermediates/share_config/default/temp-share-config.json` | Temp share config matches hvigor |
| Share config not rewritten into resources | `CompileResource.processShareConfig()` | After resource compile input is ready, write `resources/base/profile/<name>.json`; update `module.json.crossAppSharedConfig` to `$profile:<name>` | Resource profile and module reference match hvigor |
| Duplicate share URI only warns | `ProcessShareConfig.checkShareConfigObjUri()` exits | Return a hard error with hvigor-equivalent message/category | Duplicate fixture fails like hvigor |
| Startup config not called | `ProcessStartupConfig.checkAppStartupConfig()` | Add startup merge step before resource processing; read self/local/remote HAR startup configs; include ByteCode HAR preload SO json | Temp startup config matches hvigor |
| Missing startup `ohmurl` and absolute paths | `processModuleStartupConfig`, `processLocalHarDependency`, `processRemoteHarDependency` | Populate `srcEntryAbsolutePath`, `moduleName`, `startupConfigPath`, `ohmurl`, `isRemoteByteCodeHar` using package context and OHM URL utilities | Startup compile entries and temp JSON match hvigor |
| Startup resource rewrite missing | `CompileResource.processStartupConfig()` | Strip intermediate-only fields, write startup profile under resources, add `module.appStartup` when absent | Resource startup profile and final module reference match hvigor |
| Duplicate startup tasks only warn | `checkStartupTaskName()` exits | Return hard error | Duplicate startup fixture fails like hvigor |
| Privacy/PAC ignored by packager | `ProcessModulePrivacyProfile` | Validate/copy PAC/privacy profile into intermediates; add packager entry `module.privacy.json` with hvigor path/name | Privacy fixture package matches hvigor |

### D. Resource Stage

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Empty resource dependencies | hvigor restool config includes dependency resources/tables | Populate `resConfig.json.dependencies` and resource table paths from dependency graph | HSP/HAR resource fixture matches hvigor |
| Missing `hspResourcesMap` | `GenerateLoaderJson.getHspResourcesMap()` | Resolve local/remote HSP `ResourceTable.txt`; write map by dependency/module name | `loader.json.hspResourcesMap` matches |
| Missing target resource dirs | `TargetTaskService.getResourceDirs()` | Parse target `resource.directories`; validate and feed ResourceCompiler with correct order | Custom resource target fixture matches |
| Missing permission reason resource extraction | `MergeProfile.processResourceStr()` | Implement `resource_str` intermediate generation and reason filtering | Permission reason resources match |
| Missing distro/shortcut/form rewrites | `CompileResource.customizeDistroFilter()`, `customizeShortcut()`, form checks | Port these post-resource mutations using preserved profile document | Resource profile outputs match fixtures |
| Missing preload SO resource handling | `ProcessCompiledResources` and startup preload handling | Implement preload SO metadata collection and package/resource propagation | Preload SO fixture matches hvigor |

### E. `loader.json`

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Incomplete `modulePathMap` | hvigor target module map plus test/preview additions | Use effective target graph, not only workspace modules | Loader map matches |
| Partial ByteCode HAR info | `collectByteCodeInfoAndOtherCompileEntrances()` | Derive from dependency manager; include SDK stage, source maps, insight intent, other compile entrances | ByteCode HAR loader fixture matches |
| Incomplete `declarationEntry` | `processDeclarationEntry()` and runtime-only ByteCode HAR collection | Generate declaration OHM entries from ByteCode HAR deps, runtime-only packages, startup/ability entries as hvigor does | Declaration list matches |
| Empty `hspNameOhmMap` | `getHspNameOhmMap()` | Generate old/new OHM map from package context and normalized URL flag | HSP map matches |
| Empty `harNameOhmMap` | `getHarNameOhmMap()` | Generate HAR OHM map for local/remote HAR deps and dynamic deps | HAR map matches |
| Empty `updateVersionInfo` | `processUpdateVersionInfo()` | Compare ByteCode HAR dependency package versions with `pkgContextInfo` and write mismatch info | Update-version fixture matches |
| Incomplete `compileEntry` | `GenerateLoaderJson.doTaskAction()` | Add entries from insight intent, app startup config, dependency abilities, router map, runtimeOnly sources/packages, bundled deps, workers, HAR coverage | Compile entry list matches after canonical path normalization |
| Empty `otherCompileFiles` | ByteCode HAR other compile entrances | Collect files/directories according to upstream ByteCode HAR utilities | Other compile files match |
| Fixed AOT/fallback fields | `handleAotFields()` | Implement AOT strategy exactly and emit only fields hvigor emits | AOT loader JSON matches |
| `supportChunks` fixed false | hvigor project config support chunks | Parse and propagate supportChunks into loader/bundler | Multi-chunk fixture chunk list matches |

### F. Resolver / OHM URL / Dependency Graph

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| No hvigor-equivalent dependency manager | `TaskService` and dependency-manager modules | Introduce `DependencyGraphService` with dependency type, local/remote, root path, main/types, module JSON, package context, transitive traversal | Graph dump matches hvigor |
| Remote HSP/integrated HSP missing | `getHspDependencies()`, remote HSP utilities | Resolve remote HSP cache paths, signed/integrated paths, SO paths | Remote HSP fixture matches |
| Transitive runtime-only gaps | `processIndirectDependency()` | Walk dependency tree and process runtimeOnly sources/packages recursively | Dynamic import metadata matches |
| Old OHM URL mode incomplete | `generateOldOhmUrlForSo`, `getOldOhmUrlForSrcEntry` | Implement old URL generation in the same utility layer as normalized URL generation | `useNormalizedOHMUrl=false` fixture matches |
| Non-deterministic module order | Rollup `modulesById` Map order | Record resolution/load insertion order explicitly and use it for handoff sorting | `filesInfo.txt` and `modules.cache` order match |

### G. ArkTS Transform / CompileArkTS Handoff

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| ViewPU transform diffs | ets-loader transform output | Maintain a cache-TS golden suite; patch transform cases by output diff, not by runtime symptoms | Cache TS files match hvigor |
| Missing TS semantic behavior | hvigor checker/ets-loader diagnostics | Add diagnostics compatibility layer for known checks; where necessary call OXC/TS services in equivalent order | Invalid TS fixtures fail the same way |
| Plugin sequence differences | hvigor Rollup plugin stack | Document and enforce local plugin order; add per-plugin golden tests for JSON5, resources, router, module meta, commonjs | Handoff graph matches |
| Source map differences | hvigor source map generation | Compare each `sourceMaps.map` entry and offset; fix transform offsets before ABC | Source map fixture matches |
| Component collection/widget handoff differences | widget CompileArkTS task | Reuse same handoff writer and ordering rules for main and widget passes | Component collection matches |

### H. Bundle / Release / Obfuscation

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Obfuscation mutates wrong files | hvigor obfuscates final compile inputs | Run obfuscation on CompileArkTS cache TS/JS inputs or move obfuscation before handoff generation so `compile_to_abc` consumes obfuscated code | Release ABC string/name diff proves obfuscation applied |
| Obfuscation rule subset | SDK obfuscation rule engine | Either port missing rules or delegate to upstream-compatible tool for release until Rust parity exists | Obfuscation golden fixture matches |
| Terser/minifier parity | hvigor terser/minifier plugin | Add release minifier parity tests and port exact options/order | Release cache JS/TS matches |
| `supportChunks` disabled | hvigor manualChunks when enabled | Wire config flag and verify `commons`/`vendors` chunk naming | Multi-chunk output matches |
| Treeshake parity not proven | hvigor Rollup treeshake | Compare release bundle/handoff for unused exports and side-effect modules | Release transform fixture matches |

### I. ABC / ts2abc

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Custom cache format | es2abc `--cache-file` / protoBin | Implement upstream-compatible cache/protoBin layout or disable cache in parity mode | Cache files match or are intentionally absent in both builds |
| `merge_abc=false` unsupported | es2abc flag behavior | Implement non-merged outputs and ABC input handling | CLI flag fixture matches |
| Release optimizer incomplete | es2abc optimizer/register allocation | Port required optimizer passes or call upstream in parity mode for release until implemented | Release ABC matches |
| Unsupported syntax | upstream es2abc AST lowering | Add failing fixtures for each unsupported node; implement lowering or hard-error before bad output | Syntax fixtures match |
| Silent skip cases | upstream compiler either emits or errors | Replace silent skips with equivalent emission or fatal diagnostic | No unsupported fixture silently succeeds |
| Weak ABC validation | Panda file format validation | Validate magic, file size, checksum, indexes, section bounds; use `AbcReader` for ByteCode HAR checks | Corrupt ABC fixture fails like hvigor/runtime |
| Lack of byte golden tests | upstream es2abc output | Add byte-level `modules.abc` and `widgets.abc` golden comparator | ABC parity CI catches diffs |

### J. ByteCode HAR

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Filesystem scan instead of dependency graph | ByteCode HAR utilities + dependency manager | Source all ByteCode HAR data from dependency graph nodes and package context | Loader metadata order/content matches |
| Missing declaration generation | `processDeclarationEntry()` | Generate own and dependency declaration entries, including runtime-only ByteCode HARs | Declaration fixture matches |
| Missing runtime package verification | Runtime uses `byteCodeHarInfo` paths | Package/load fixture that installs ByteCode HAR dependency and validates runtime lookup | E2E ByteCode HAR fixture passes |
| Empty `updateVersionInfo` | `processUpdateVersionInfo()` | Implement package version comparison for ByteCode HAR dependency metadata | Update info matches |

### K. Widget / Form

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Widget ABC cache not separated enough | hvigor widget compile temp/cache paths | Use `arkdown-abc::compile_widget()` or equivalent config with distinct ABC cache dir and widget-specific options | Widget cache files and `widgets.abc` match |
| Widget loader incomplete | hvigor writes `widget_loader.json` from same loader data | Generate widget loader after all loader fields are complete | Widget loader JSON matches |
| Form validation partial | `TargetTaskService.getFormJsonArr()` and form schema checks | Port metadata/resource checks and form profile source handling | Bad form fixtures fail like hvigor |

### L. Packaging / Signing / Native

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| HSP/HAR package modes partial | hvigor HSP/HAR tasks | Add target-specific pack inputs, HAR module-info flow, source HAR vs ByteCode HAR modes | HSP/HAR archives match |
| Integrated HSP not passed | `ProcessIntegratedHsp` | Wire normalized integrated HSP entries into CLI pack input | Package contains same integrated HSP entries |
| Privacy not written | Packager should include processed privacy profile | Add zip entry `module.privacy.json` with exact compression/order | Privacy package matches |
| Native build missing | hvigor CMake/native tasks | Add native build invocation or artifact import mode; collect ABI-specific `.so` outputs | Native fixture package matches |
| Preload SO missing | `preloadSo` and startup/system preload tasks | Generate preload metadata and package SOs exactly | Preload fixture matches |
| Pack info defaults | hvigor pack info task | Generate pack.info from effective app/module/dependency data, not fixed defaults | `pack.info` matches |
| Signing byte stability | hvigor signing task | Separate unsigned byte parity from signing; for signed parity, control certs/timestamps/profile and compare signing block semantics | Signed fixture passes semantic verifier |

### M. Cache / Incremental

| Gap | Upstream source of truth | Alignment implementation | Acceptance |
| --- | --- | --- | --- |
| Coarse task cache only | hvigor task inputs + cache invalidators | Track the same declared inputs and output existence per task | Rebuild/skip decisions match scenario tests |
| No Rollup msgpack cache | `RollupCacheService` | Implement compatible cache read/write or disable in parity mode; never produce stale output | Incremental fixture matches |
| Incomplete invalid-cache behavior | hvigor invalid-cache plugin | Port invalidation for source/profile/router/obfuscation/module JSON changes | Stale file fixture matches |
| Custom ABC cache | es2abc cache/protoBin | See ABC section; keep cache key compatible with upstream flags and package entries | Incremental ABC fixture matches |

## Repair And Alignment Plan

### Phase 0: Build The Hvigor Oracle Harness First

Do this before large fixes, otherwise "aligned" cannot be verified.

Deliverables:

- A script/test runner that builds the same fixture with hvigor and arkdown from
  clean output directories.
- Artifact collector for:
  `merge_profile`, `process_profile`, `res`, `loader`, `loader_out`,
  `source_map`, `importee`, `package`, `hap_metadata`, `outputs`.
- Diff tools:
  JSON canonical diff, zip entry order/method/content diff, resource index diff,
  CompileArkTS cache TS diff, ABC section diff, package metadata diff.
- Fixture matrix:
  minimal HAP debug/release, HSP, source HAR, ByteCode HAR, runtimeOnly,
  dynamic import, router map, share config, startup config, privacy/PAC,
  widget/form, native/preload SO, integrated HSP, normalized OHM on/off,
  release obfuscation, supportChunks.

Exit criterion:

- CI can report exact artifact mismatch by stage and file.

### Phase 1: Fix Profile Architecture

Recommended implementation:

- Keep a raw `serde_json::Value` or `IndexMap`-preserving representation for
  merged profile objects.
- Implement hvigor-style mutate-in-place profile processing:
  start from app/module JSON, then apply virtualMachine, compileMode,
  deviceTypes, dependencies, packageName, build mode, API fields, bundle type,
  resource IDs, and product overrides.
- Keep typed helpers only for fields that drive compilation, not as the only
  storage of profile output.
- Change `crossAppSharedConfig` input handling to support `$profile:<name>` and
  final resource profile references.
- Add field-preservation tests with deliberately unknown but schema-valid fields.

Exit criterion:

- `module.json`, `merge_profile/default/module.json`,
  `process_profile/default/module.json`, and `ark_module.json` match hvigor for
  profile-heavy fixtures.

### Phase 2: Wire Share, Startup, Privacy, And Resource Rewrites

Tasks:

- Call share/startup merge tasks from the CLI in hvigor order.
- Match hvigor fatal validation for duplicate share URI and startup task names.
- Fill startup `ohmurl`, `srcEntryAbsolutePath`, `moduleName`,
  `startupConfigPath`, and preload SO task fields.
- During resource processing, write profile JSON under
  `res/default/resources/base/profile` and update `module.json` references.
- Implement PAC/privacy profile validation and package `module.privacy.json`.

Exit criterion:

- Share/startup/privacy fixtures match hvigor intermediates and final package.

### Phase 3: Complete Dependency And Loader Parity

Tasks:

- Build a dependency graph layer equivalent to hvigor dependency-manager:
  local/remote HAR, HSP, ByteCode HAR, SO, dynamic deps, transitive deps.
- Populate `hspNameOhmMap`, `harNameOhmMap`, `hspResourcesMap`,
  `updateVersionInfo`, `otherCompileFiles`, complete `compileEntry`, and AOT
  fields.
- Implement `sourceOption.workers`, metadata workers, insight intent, dependency
  ability entries, startup entries, and bundled dependency entries.
- Add normalized and old OHM URL golden tests.

Exit criterion:

- `loader.json`, `pkgContextInfo.json`, importee caches, and CompileArkTS entry
  lists match hvigor for dependency-heavy fixtures.

### Phase 4: Stabilize CompileArkTS Handoff

Tasks:

- Make module ordering match hvigor Rollup `Map` insertion order.
- Continue closing ViewPU/ArkUI transform diffs using cache TS golden tests.
- Add source map golden comparison for every transformed file.
- Ensure release obfuscation/minification is applied to the same files that are
  passed to ABC.

Exit criterion:

- `filesInfo.txt`, `modules.cache`, `npmEntries.txt`, cache TS, source maps, and
  component collections match hvigor for real-world fixtures.

### Phase 5: Complete ABC Backend Parity

Tasks:

- Implement upstream-compatible cache/protoBin behavior or gate it explicitly.
- Implement `merge_abc=false` and ABC input handling.
- Complete release optimization behavior for `opt_level=2`.
- Fix unsupported syntax and silent-skip cases.
- Strengthen `AbcReader` validation.
- Add byte-level ABC golden tests beyond smoke/opcode tests.

Exit criterion:

- `modules.abc` and `widgets.abc` are byte-identical for the fixture matrix, or
  documented non-byte-stable fields are normalized by an accepted comparator.

### Phase 6: Complete Release, Package, Native, And Signing

Tasks:

- Wire `supportChunks` from config and verify manual chunks.
- Match hvigor obfuscation rule behavior or call the upstream-compatible tool.
- Implement native/CMake output collection, preload SO, integrated HSP, HSP/HAR
  packaging, source/ByteCode HAR variants, pack info fields, and privacy files.
- Add deterministic unsigned package comparison.
- Define signing parity as either byte-identical with controlled inputs or
  semantic verification.

Exit criterion:

- Unsigned HAP/HSP/HAR archives match hvigor by zip entry list, compression
  method, CRC, size, and content.

### Phase 7: CI Gate

Tasks:

- Add a "hvigor parity" test target that can be run locally and in CI when SDK
  paths are available.
- Block changes that regress any stage-level golden output.
- Track every known mismatch in a machine-readable allowlist with owner and
  expiry.

Exit criterion:

- No "full hvigor parity" claim is made unless the allowlist is empty for the
  declared fixture matrix.

## Immediate Fix Priority

Recommended order:

1. Replace profile whitelist reconstruction with preserve-and-mutate output.
2. Wire share/startup/privacy and resource-phase profile rewrites.
3. Populate loader dependency maps from a real hvigor-like dependency graph.
4. Fix release obfuscation so it affects actual ABC inputs.
5. Stabilize CompileArkTS handoff order and ViewPU cache TS parity.
6. Expand ABC byte-level golden coverage and close backend gaps.
7. Finish package/native/HSP/HAR/ByteCode HAR end-to-end parity.

Until phases 1-4 are complete, final ABC/package parity on non-trivial projects
cannot be guaranteed even if the Rust ABC emitter itself is improved.
