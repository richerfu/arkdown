# TEST-E2E: hmosworld End-to-End Build and Device Verification

**Project**: `https://github.com/susns/HmosWorld`
**Subdirectory**: `HMOSWorld/Application`
**Purpose**: The golden-path integration test. A full arkdown build of a real, multi-module
HarmonyOS project that produces valid HAR/HAP/APP artifacts and runs correctly on a HOS device.

---

## Project Structure (from source)

```
HMOSWorld/Application/
├── build-profile.json5          SDK 5.0.0(12), product "default"
├── oh-package.json5             deps: @hw-agconnect/cloud, @hw-agconnect/hmcore, long
├── products/
│   └── phone/                   type: entry  → phone.hap
│       └── module.json5         mainElement: EntryAbility
│                                extensionAbilities: EntryFormAbility (ArkTS widget)
│                                permissions: INTERNET, LOCATION, KEEP_BACKGROUND_RUNNING, ...
├── features/
│   ├── login/                   type: har
│   ├── discover/                type: har
│   ├── learning/                type: har
│   ├── topic/                   type: har
│   ├── challenge/               type: har
│   ├── mine/                    type: har
│   └── conference/              type: har
└── commons/
    ├── uicomponents/            type: har
    ├── utils/                   type: har
    ├── network/                 type: har
    ├── datastore/               type: har
    ├── audioplayer/             type: har
    └── aspect/                  type: har
```

---

## Test File Layout

```
tests/integration/
├── e2e_hmosworld.rs
│   (top-level orchestrator — runs phases in order, fails fast)
├── phase_1_setup.rs
├── phase_2_har_build.rs
├── phase_3_hap_build.rs
├── phase_4_app_build.rs
└── phase_5_device.rs
    (requires ARKDOWN_TEST_DEVICE env var — skipped if not set)
```

---

## Prerequisites

```bash
# Clone the test fixture (run once)
git clone https://github.com/susns/HmosWorld tests/fixtures/hmosworld

# Set SDK location for ets-loader transform
export OHOS_SDK_HOME=/path/to/sdk/default/openharmony

# For device tests only:
export ARKDOWN_TEST_DEVICE=<hdc device serial>   # e.g. "127.0.0.1:5555"
```

---

## Phase 1: Setup

**File**: `phase_1_setup.rs`

### TC-001: fixture directory exists and contains expected structure

- **Expected**: `tests/fixtures/hmosworld/HMOSWorld/Application/build-profile.json5` exists
- **On failure**: print instructions to clone the fixture; skip all subsequent phases

### TC-002: `build-profile.json5` parses without error

- **Expected**: `ArkdownConfig::from_project("tests/fixtures/hmosworld/HMOSWorld/Application", ...)`
  returns `Ok(config)`

### TC-003: config contains 14 modules (1 entry + 7 features + 6 commons)

- **Expected**: `config.modules.len() == 14`

### TC-004: config `compile_api_version` is 12

- **Expected**: `config.sdk_config.compile_api_version == 12`

### TC-005: all module source directories exist

- **Expected**: every `ModuleConfig.src_path` points to an existing directory

### TC-006: ohpm dependencies resolved

- **Expected**: `oh-package.json5` dependency graph resolved;
  external packages (`@hw-agconnect/cloud`, `long`) identified

---

## Phase 2: HAR Build

**File**: `phase_2_har_build.rs`

Build each of the 12 HAR modules independently. Each produces a `.har` archive.

### TC-007: `commons/utils` builds to a valid HAR

- **Command**: `arkdown build --target har --module utils`
- **Expected artifacts**:
  - `build/default/outputs/default/utils.har` exists and is non-empty
  - ZIP contains `oh-package.json5` at root
  - ZIP contains `.ets` source files under `src/main/ets/`
  - ZIP contains `resources.index` (if `resources/` present)

### TC-008: `commons/uicomponents` builds to a valid HAR

- **Expected**: `uicomponents.har` produced; ZIP contains expected `.ets` entries

### TC-009: `commons/network` builds to a valid HAR

- **Expected**: `network.har` produced

### TC-010: `commons/datastore` builds to a valid HAR

- **Expected**: `datastore.har` produced

### TC-011: `commons/audioplayer` builds to a valid HAR

- **Expected**: `audioplayer.har` produced

### TC-012: `commons/aspect` builds to a valid HAR

- **Expected**: `aspect.har` produced

### TC-013: `features/login` builds to a valid HAR

- **Expected**: `login.har` produced

### TC-014: `features/discover` builds to a valid HAR

- **Expected**: `discover.har` produced

### TC-015: `features/learning` builds to a valid HAR

- **Expected**: `learning.har` produced

### TC-016: `features/topic` builds to a valid HAR

- **Expected**: `topic.har` produced

### TC-017: `features/challenge` builds to a valid HAR

- **Expected**: `challenge.har` produced

### TC-018: `features/mine` builds to a valid HAR

- **Expected**: `mine.har` produced

### TC-019: `features/conference` builds to a valid HAR

- **Expected**: `conference.har` produced

### TC-020: no duplicate filenames across all 12 HARs

- **Expected**: `check-duplicate-filenames-plugin` passes for all HAR builds (no conflicts)

---

## Phase 3: HAP Build

**File**: `phase_3_hap_build.rs`

Build the `phone` entry module into a deployable `.hap`.

### TC-021: HAP build completes without error

- **Command**: `arkdown build --target hap --module phone`
- **Expected**: exits 0; no error output

### TC-022: `phone.hap` artifact exists and is non-empty

- **Expected**: `build/default/outputs/default/phone.hap` exists; size > 100KB

### TC-023: HAP ZIP contains `module.json` at root

- **Expected**: `zipfile.namelist()` contains `"module.json"`

### TC-024: `module.json` has correct `virtualMachine` field

- **Expected**: `module.json#module.virtualMachine` matches `"ark<version>"` pattern

### TC-025: `module.json` has correct `compileMode`

- **Expected**: `module.json#module.compileMode == "esmodule"`

### TC-026: HAP ZIP contains `modules.abc` (merged bytecode)

- **Expected**: `"modules.abc"` present in ZIP

### TC-027: `modules.abc` is non-empty valid Panda bytecode

- **Expected**: first 4 bytes of `modules.abc` match Panda magic bytes
  (from `arkcompiler_runtime_core` format spec)

### TC-028: HAP ZIP contains `resources.index`

- **Expected**: `"resources.index"` present in ZIP

### TC-029: HAP ZIP contains `module.json`

- **Expected**: `"module.json"` present at root

### TC-030: HAP entry ability present in `module.json`

- **Expected**: `module.json#module.abilities[0].name == "EntryAbility"`
  with `srcEntry` set

### TC-031: ArkTS widget form ability present

- **Expected**: `module.json#module.extensionAbilities` contains
  `{ name: "EntryFormAbility", type: "form" }`

### TC-032: all 6 permissions declared in `module.json`

- **Expected**: `module.json#module.requestPermissions` contains
  `ohos.permission.INTERNET`, `ohos.permission.LOCATION`,
  `ohos.permission.KEEP_BACKGROUND_RUNNING`, and the others declared in source

### TC-033: `.abc` not compressed (stored method in ZIP)

- **Expected**: `modules.abc` zip entry has compression method `Stored` (0), not `Deflated` (8)

### TC-034: debug build includes debug info in `.abc`

- **Command**: `arkdown build --target hap --mode debug`
- **Expected**: `modules.abc` contains debug section (non-zero `DebugInfoSection` offset in header)

### TC-035: release build applies tree shaking

- **Command**: `arkdown build --target hap --mode release`
- **Expected**: `modules.abc` smaller than debug build (dead code removed)

### TC-036: release build output size comparable to hvigorw baseline

- **Baseline**: reference `phone.hap` size from a known hvigorw build
- **Expected**: arkdown release `.hap` within 20% of hvigorw baseline size
- **Note**: exact parity not required — tree shaking and optimization may differ

---

## Phase 4: APP Build

**File**: `phase_4_app_build.rs`

The `.app` is the multi-HAP bundle for app store distribution. For hmosworld (single entry
HAP), the app contains the signed `phone.hap`.

### TC-037: APP build completes without error

- **Command**: `arkdown build --target app`
- **Expected**: exits 0

### TC-038: `.app` artifact exists

- **Expected**: `build/default/outputs/default/hmosworld.app` or similar output

### TC-039: `.app` ZIP contains `phone.hap`

- **Expected**: `hmosworld.app` is a ZIP containing `phone.hap`

### TC-040: `.app` contains `pack.info` manifest

- **Expected**: ZIP entry `"pack.info"` present with bundle/module metadata

---

## Phase 5: Device Verification

**File**: `phase_5_device.rs`
**Gate**: `#[cfg(feature = "device-tests")]`, requires `ARKDOWN_TEST_DEVICE` env var.
**Device tool**: `hdc` (HarmonyOS Device Connector)

These tests run on a real HOS device connected via USB or TCP. They verify that arkdown's
output artifacts are not just structurally valid but actually execute correctly on ArkRuntime.

### TC-041: device is reachable via hdc

- **Command**: `hdc -t $ARKDOWN_TEST_DEVICE list targets`
- **Expected**: device serial appears in output

### TC-042: debug HAP installs successfully

- **Command**: `hdc -t $DEVICE install build/default/outputs/default/phone.hap`
- **Expected**: output contains `"successfully"` or `"success"` (case-insensitive)

### TC-043: app launches without crash

- **Command**:
  ```bash
  hdc -t $DEVICE shell aa start \
    -b com.example.hmosworld \
    -a EntryAbility
  ```
- **Expected**: command exits 0; no `"Error"` in output

### TC-044: app process is running after launch

- **Command**: `hdc -t $DEVICE shell ps -ef | grep hmosworld`
- **Expected**: process entry for `com.example.hmosworld` present

### TC-045: no fatal crash within 5 seconds of launch

- **Command**: `hdc -t $DEVICE shell hilog -T HiCrashCatcher | timeout 5 grep hmosworld`
- **Expected**: no crash report for `com.example.hmosworld` in the 5-second window

### TC-046: release HAP installs successfully

- **Command**: build with `--mode release`, then install
- **Expected**: same as TC-042

### TC-047: release app launches without crash

- **Expected**: same as TC-043 for the release build

### TC-048: uninstall succeeds

- **Command**: `hdc -t $DEVICE shell pm uninstall com.example.hmosworld`
- **Expected**: exits 0; app removed from device

---

## Output Artifact Checklist

After a complete `arkdown build --target app` run, verify these files exist:

```
build/default/outputs/
├── har/
│   ├── utils.har
│   ├── uicomponents.har
│   ├── network.har
│   ├── datastore.har
│   ├── audioplayer.har
│   ├── aspect.har
│   ├── login.har
│   ├── discover.har
│   ├── learning.har
│   ├── topic.har
│   ├── challenge.har
│   ├── mine.har
│   └── conference.har
├── default/
│   └── phone.hap
└── default/
    └── hmosworld.app
```

---

## Failure Triage Guide

| Symptom                                | Likely cause                               | Check                                    |
| -------------------------------------- | ------------------------------------------ | ---------------------------------------- |
| HAR build fails                        | Missing source files or unresolved imports | `ArkdownError::ResolutionFailed` message |
| `modules.abc` invalid magic            | IR lowering bug in `arkdown-abc`           | TEST-009 IR tests                        |
| `module.json` missing `virtualMachine` | Profile processor bug                      | TEST-010 TC-001                          |
| Install fails: "signature error"       | HAP unsigned or wrong profile              | SPEC-021 signing                         |
| Install fails: "incompatible API"      | Wrong `compile_api_version`                | TEST-002 TC-003                          |
| App crashes on launch                  | Runtime error in bundled `.abc`            | Enable debug info, use `hdc shell hilog` |
| `.abc` larger than hvigorw             | Tree shaking not active in release         | TEST-005 TC-005                          |

---

## Design References

- `meta/design/pipeline.md` — full build pipeline (all phases exercised by this test)
- `meta/design/chunk-model.md` — HAP ZIP structure
- `meta/design/abc-pipeline.md` — Panda bytecode format (TC-027)
- `meta/design/module-resolution.md` — `file:` HAR resolution (12 local HARs)
- `meta/design/hvigorw-analysis.md` — baseline behavior to compare against
