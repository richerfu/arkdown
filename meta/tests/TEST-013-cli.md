# TEST-013: packages/arkdown

**Workspace package**: `arkdown-napi` (`packages/arkdown`)

---

## Test File Layout

```
packages/arkdown/src/
├── cmd/
└── main.rs
packages/arkdown/tests/
├── arg_parsing.rs        ← CLI flag → config mapping
├── exit_codes.rs         ← exit code on success/failure
└── hvigorw_compat.rs     ← hvigorw wrapper script argument translation
```

---

## arg_parsing.rs

### TC-001: `build --target hap` sets `OutputTarget::Hap`

- **Input args**: `["build", "--target", "hap"]`
- **Expected**: parsed config has `output_target = OutputTarget::Hap`

### TC-002: `build --mode release` sets `BuildMode::Release`

- **Input args**: `["build", "--mode", "release"]`
- **Expected**: `build_mode = BuildMode::Release`

### TC-003: default mode is `debug`

- **Input args**: `["build", "--target", "hap"]` (no `--mode`)
- **Expected**: `build_mode = BuildMode::Debug`

### TC-004: `--module entry` scopes to single module

- **Input args**: `["build", "--target", "hap", "--module", "entry"]`
- **Expected**: only module named `"entry"` built

### TC-005: `--product default` selects product

- **Input args**: `["build", "--product", "default"]`
- **Expected**: product `"default"` selected

### TC-006: unknown `--target` value → error with usage

- **Input args**: `["build", "--target", "invalid"]`
- **Expected**: exits with code 2, stderr contains usage hint

---

## exit_codes.rs

### TC-007: successful build exits 0

- **Setup**: mock build that succeeds
- **Expected**: exit code 0

### TC-008: build error exits 1

- **Setup**: mock build that returns `ArkdownError`
- **Expected**: exit code 1

### TC-009: config error (bad project) exits 2

- **Setup**: `--project` pointing to empty directory
- **Expected**: exit code 2

---

## hvigorw_compat.rs

These verify the shell wrapper script translation logic (embedded in CLI or tested separately).

### TC-010: `assembleHap` translates to `build --target hap`

- **Input**: wrapper called with `assembleHap`
- **Expected**: arkdown invoked with `build --target hap`

### TC-011: `assembleHap --mode release` passes mode correctly

- **Expected**: arkdown invoked with `build --target hap --mode release`

### TC-012: `-m entry` translates to `--module entry`

- **Input**: `assembleHap -m entry`
- **Expected**: arkdown invoked with `build --target hap --module entry`

### TC-013: unknown hvigorw command exits 2 with error message

- **Input**: `./hvigorw unknownTask`
- **Expected**: exit 2, stderr explains supported commands
