# arkdown Test Documentation

One file per crate or test scope. Each file specifies the test file layout, every test case
(with input and expected output), and — for crates that rewrite a hvigor JS plugin — the
exact behavioral parity cases derived from reading the original source.

## Convention

- Unit tests live in `crates/<name>/src/tests/` as separate modules per logical concern.
  Never put all tests in one file; split by the behavior being tested.
- Integration tests that span multiple crates live in `tests/integration/`.
- Device tests require `#[cfg(feature = "device-tests")]` and `ARKDOWN_TEST_DEVICE` env var.
- Tests that require the ArkTS transform subprocess (`ets-loader`) are gated with
  `#[cfg(feature = "ets-loader-tests")]` and require `OHOS_SDK_HOME` to be set.
  `es2abc` and `restool` are pure Rust — no feature gate needed for those.
- Snapshot tests (for generated JSON, loader.json, module.json) use `insta`.

## Files

| File                                      | Crate                                                              | Notes                                                   |
| ----------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------- |
| [TEST-001](TEST-001-core-types.md)        | `arkdown-core`                                                     | Type compilation, error display                         |
| [TEST-002](TEST-002-config.md)            | `arkdown-config`                                                   | JSON5 parsing, SDK discovery                            |
| [TEST-003](TEST-003-resolver.md)          | `arkdown-resolver`                                                 | All OHOS specifier forms                                |
| [TEST-004](TEST-004-loader.md)            | `arkdown-loader`                                                   | Plugin chain, parallel load                             |
| [TEST-005](TEST-005-bundler.md)           | `arkdown`                                                          | Link, tree-shake, chunk split                           |
| [TEST-006](TEST-006-chunk-wrapper.md)     | `arkdown-plugin-chunk-wrapper`                                     | **Original behavioral parity**                          |
| [TEST-007](TEST-007-json5.md)             | `arkdown-plugin-json5`                                             | **Original behavioral parity**                          |
| [TEST-008](TEST-008-duplicate-check.md)   | `arkdown-plugin-duplicate-check`                                   | **Original behavioral parity**                          |
| [TEST-009](TEST-009-abc-compiler.md)      | `arkdown-abc`                                                      | **Original behavioral parity** (Rust ABC compiler)      |
| [TEST-010](TEST-010-profile.md)           | `arkdown-profile`                                                  | loader.json generation                                  |
| [TEST-011](TEST-011-resource.md)          | `arkdown-resource`                                                 | **Original behavioral parity** (Rust resource compiler) |
| [TEST-012](TEST-012-packager.md)          | `arkdown-pack`                                                     | ZIP structure correctness                               |
| [TEST-013](TEST-013-cli.md)               | `arkdown-cli`                                                      | Command parsing, error codes                            |
| [TEST-014](TEST-014-module-meta.md)       | `arkdown-plugin-module-meta`                                       | **Original behavioral parity**                          |
| [TEST-015](TEST-015-ohos-resolve.md)      | `arkdown-plugin-ohos-resolve`                                      | **Original behavioral parity**                          |
| [TEST-016](TEST-016-har-dedup.md)         | `arkdown-plugin-har-dedup`                                         | **Original behavioral parity**                          |
| [TEST-017](TEST-017-019-small-plugins.md) | `arkdown-plugin-router-map` / `-sensitive-check` / `arkdown-cache` | Cache invalidation, case check                          |
| **[TEST-E2E](TEST-E2E-hmosworld.md)**     | all crates                                                         | **Golden path: hmosworld HAR/HAP/APP/device**           |

## End-to-End Test

[TEST-E2E-hmosworld.md](TEST-E2E-hmosworld.md) is the definitive acceptance test. It:

1. Builds all 12 HARs from `hmosworld/HMOSWorld/Application` (6 commons + 7 features)
2. Builds the `phone` entry HAP (48 TC covering ZIP structure, `.abc` validity, permissions)
3. Builds the `.app` bundle (TC-037 – TC-040)
4. Installs and runs on a real HOS device via `hdc` (TC-041 – TC-048, requires device)

**A passing TEST-E2E is the definition of "arkdown works".**

Clone the fixture:

```bash
git clone https://gitcode.com/HarmonyOS_Samples/hmosworld tests/fixtures/hmosworld
```

## Origin Behavioral Parity

Crates marked **"Original behavioral parity"** rewrite a hvigor JavaScript plugin. Their tests
are derived by reading the original compiled JS source. The test cases in those files are
the ground truth — passing all of them is equivalent to passing the original plugin's test suite.

Original source locations:

- `wrapper-chunk-plugin` → hvigor-arkts-compose `dist/src/plugins/wrapper-chunk-plugin.js`
- `resolve/index` → hvigor-arkts-compose `dist/src/plugins/resolve/`
- `module-meta-info-plugin` → hvigor-arkts-compose `dist/src/plugins/module-meta-info-plugin.js`
- `har-deduplicate` → hvigor-arkts-compose `dist/src/plugins/har-deduplicate/`
- `check-duplicate-filenames-plugin` → hvigor-arkts-compose `dist/src/plugins/check-duplicate-filenames-plugin.js`
- `es2abc` (ABC compiler) → `https://gitcode.com/openharmony/arkcompiler_ets_frontend`
- `restool` (resource compiler) → `https://gitcode.com/openharmony/developtools_restool`
