# SPEC-045: Privacy Profile Embedding

**Crate**: `arkdown-pack`
**Priority**: P2 — privacy-sensitive apps
**Blocked by**: SPEC-012 (packager)
**Blocks**: —

---

## Summary

If `src/main/resources/base/profile/privacy.json` exists in the module source tree,
validate it and embed it as `module.privacy.json` in the HAP archive. This mirrors
hvigor's `ProcessModulePrivacyProfile` task (`process-module-privacy-profile.js`).

---

## Background: What hvigor Does

From `process-module-privacy-profile.js`:

```javascript
class ProcessModulePrivacyProfile extends AbstractTask {
    async doTaskAction() {
        const privacyJsonPath = path.resolve(
            this.moduleSrcMainPath,
            "resources/base/profile/privacy.json"
        );

        if (!fse.existsSync(privacyJsonPath)) return; // optional file

        // Validate the privacy.json structure
        const privacyJson = fse.readJsonSync(privacyJsonPath);
        this.validatePrivacyProfile(privacyJson);

        // Copy to intermediates as module.privacy.json
        const outputPath = path.resolve(
            this.intermediatesDir,
            "module.privacy.json"
        );
        fse.copySync(privacyJsonPath, outputPath);
    }

    validatePrivacyProfile(profile) {
        // Validates that required fields are present
        // Throws if the profile is malformed
        if (!profile.dataAndPrivacy) {
            throw new Error("privacy.json must contain 'dataAndPrivacy' field");
        }
    }
}
```

The `module.privacy.json` is then embedded in the HAP zip by `PackageHap`:

```javascript
// In PackageHap.doTaskAction():
const privacyJsonPath = path.resolve(intermediatesDir, "module.privacy.json");
if (fse.existsSync(privacyJsonPath)) {
    zip.addFile("module.privacy.json", privacyJsonPath);
}
```

The `privacy.json` format (from OHOS documentation):

```json
{
    "dataAndPrivacy": {
        "dataCollected": [
            {
                "dataType": "location",
                "dataLabel": "precise_location",
                "dataUsed": ["app_functionality"],
                "dataShared": false
            }
        ]
    }
}
```

---

## Scope

### 1. `PrivacyProfile` Struct

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyProfile {
    pub data_and_privacy: DataAndPrivacy,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataAndPrivacy {
    #[serde(default)]
    pub data_collected: Vec<DataCollectedEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCollectedEntry {
    pub data_type: String,
    pub data_label: String,
    pub data_used: Vec<String>,
    pub data_shared: bool,
}
```

### 2. `process_privacy_profile()` Function

```rust
/// Process the module privacy profile.
///
/// Reads `<module_src_main>/resources/base/profile/privacy.json` if it exists,
/// validates it, and writes it to `<intermediates>/module.privacy.json`.
///
/// Returns `Ok(true)` if the file was processed, `Ok(false)` if it does not exist.
pub fn process_privacy_profile(
    module_src_main: &Path,
    intermediates_dir: &Path,
) -> Result<bool, PackError> {
    let privacy_json_path = module_src_main
        .join("resources/base/profile/privacy.json");

    if !privacy_json_path.exists() {
        return Ok(false); // Optional file — not an error
    }

    let content = std::fs::read_to_string(&privacy_json_path)?;
    let profile: PrivacyProfile = serde_json::from_str(&content)
        .map_err(|e| PackError::InvalidPrivacyProfile {
            path: privacy_json_path.clone(),
            reason: e.to_string(),
        })?;

    // Validate: dataAndPrivacy must be present (enforced by struct deserialization)
    // Additional validation can be added here

    let output_path = intermediates_dir.join("module.privacy.json");
    std::fs::write(&output_path, &content)?;

    Ok(true)
}
```

### 3. Error Type Extension

Add to `PackError`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    // ... existing variants ...
    #[error("invalid privacy profile at {path}: {reason}")]
    InvalidPrivacyProfile { path: PathBuf, reason: String },
}
```

### 4. HAP Packaging Integration

In `arkdown-pack::pack_hap()`, include `module.privacy.json` when present:

```rust
pub fn pack_hap(config: &PackConfig) -> Result<PathBuf, PackError> {
    // ... existing processing ...

    // Process privacy profile (optional)
    process_privacy_profile(&config.module_src_main, &config.intermediates_dir)?;

    // ... zip assembly ...
    let privacy_json = config.intermediates_dir.join("module.privacy.json");
    if privacy_json.exists() {
        zip_writer.add_file("module.privacy.json", &privacy_json)?;
    }

    // ...
}
```

### 5. CLI Integration

`process_privacy_profile()` is called as part of `pack_hap()` — no separate CLI step
is needed. The function is a no-op when `privacy.json` does not exist.

---

## Files to Change

```
crates/arkdown-pack/src/
├── privacy_profile.rs  — NEW: PrivacyProfile structs, process_privacy_profile()
└── lib.rs              — call process_privacy_profile() in pack_hap();
                          include module.privacy.json in zip;
                          add InvalidPrivacyProfile to PackError
```

---

## Test Plan

- Module without `privacy.json`: `process_privacy_profile()` returns `Ok(false)`;
  HAP zip does not contain `module.privacy.json`.
- Module with valid `privacy.json`: `process_privacy_profile()` returns `Ok(true)`;
  `<intermediates>/module.privacy.json` is written; HAP zip contains
  `module.privacy.json` with the correct content.
- `privacy.json` missing `dataAndPrivacy` field: `process_privacy_profile()` returns
  `Err(InvalidPrivacyProfile)`.
- `privacy.json` with empty `dataCollected` array: valid; `process_privacy_profile()`
  returns `Ok(true)`.
- `privacy.json` is not valid JSON: `process_privacy_profile()` returns
  `Err(InvalidPrivacyProfile)`.
- `pack_hap()` output: HAP zip entry `module.privacy.json` is byte-identical to the
  source `privacy.json`.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/process-module-privacy-profile.js`
  — `doTaskAction()`, `validatePrivacyProfile()`
- `meta/tasks/SPEC-012-packager.md` — HAP zip structure, `pack_hap()`
- `meta/design/hvigorw-analysis.md` §10 — HAP canonical entries
