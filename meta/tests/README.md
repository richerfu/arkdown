# arkdown Test Documentation

One file per crate or test scope. Each file specifies the test file layout,
every test case (with input and expected output), and—for internal Rolldown
adapters that replace a hvigor JavaScript plugin—the exact behavioral parity
cases derived from the installed IDE source.

## Convention

- Unit tests live in `crates/<name>/src/tests/` as separate modules per logical concern.
  Never put all tests in one file; split by the behavior being tested.
- Integration tests that span multiple crates live in `tests/integration/`.
- Device tests require `#[cfg(feature = "device-tests")]` and `ARKDOWN_TEST_DEVICE` env var.
- ArkTS transform and bundling tests do not require an OHOS SDK. End-to-end
  tests that exercise SDK-native backends such as `es2abc`, `restool`, or
  signing tools must declare the required SDK environment explicitly.
- Snapshot tests (for generated JSON, loader.json, module.json) use `insta`.

## Files

| File                                      | Crate                                                              | Notes                                                   |
| ----------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------- |
| [TEST-001](TEST-001-core-types.md)        | `arkdown-core`                                                     | Type compilation, error display                         |
| [TEST-002](TEST-002-config.md)            | `arkdown-config`                                                   | JSON5 parsing, SDK discovery                            |
| [TEST-003](TEST-003-resolver.md)          | `arkdown-resolver`                                                 | All OHOS specifier forms                                |
| [TEST-005](TEST-005-bundler.md)           | `arkdown`                                                          | Link, tree-shake, chunk split                           |
| [TEST-007](TEST-007-json5.md)             | `arkdown::plugins::json5`                                          | **Installed-IDE behavioral parity**                     |
| [TEST-008](TEST-008-duplicate-check.md)   | `arkdown::plugins::duplicate_check`                                | **Installed-IDE behavioral parity**                     |
| [TEST-009](TEST-009-abc-compiler.md)      | `arkdown-abc` (planned; SDK `es2abc` backend today)                | **Original behavioral parity** (Rust ABC compiler)      |
| [TEST-010](TEST-010-profile.md)           | `arkdown-profile`                                                  | loader.json generation                                  |
| [TEST-011](TEST-011-resource.md)          | `arkdown-resource`                                                 | **Original behavioral parity** (Rust resource compiler) |
| [TEST-012](TEST-012-packager.md)          | `arkdown-pack`                                                     | ZIP structure correctness                               |
| [TEST-013](TEST-013-cli.md)               | `packages/arkdown`                                                      | Command parsing, error codes                            |
| [TEST-014](TEST-014-module-meta.md)       | `arkdown::plugins::module_meta`                                    | **Installed-IDE behavioral parity**                     |
| [TEST-015](TEST-015-ohos-resolve.md)      | `arkdown::plugins::ohos_resolve` + `arkdown-resolver`              | **Installed-IDE behavioral parity**                     |
| [TEST-016](TEST-016-har-dedup.md)         | `arkdown::plugins::har_dedup`                                      | **Installed-IDE behavioral parity**                     |
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
git clone https://github.com/susns/HmosWorld tests/fixtures/hmosworld/HMOSWorld
```

## Origin Behavioral Parity

Scopes marked **"Installed-IDE behavioral parity"** replace a hvigor
JavaScript plugin inside Arkdown's Rolldown plugin directory. Their tests are
derived from the latest installed compiled source and generated artifacts.
There are no standalone `arkdown-plugin-*` crates in the final architecture.

Original source locations:

- `resolve/index` → hvigor-arkts-compose `dist/src/plugins/resolve/`
- `module-meta-info-plugin` → hvigor-arkts-compose `dist/src/plugins/module-meta-info-plugin.js`
- `har-deduplicate` → hvigor-arkts-compose `dist/src/plugins/har-deduplicate/`
- `check-duplicate-filenames-plugin` → hvigor-arkts-compose `dist/src/plugins/check-duplicate-filenames-plugin.js`
- `router-map-plugin` behavior is owned by `arkdown-profile` and cache fingerprints.
- `use-sensitive-check-plugin` behavior is owned by `arkdown-resolver::case_check`.
