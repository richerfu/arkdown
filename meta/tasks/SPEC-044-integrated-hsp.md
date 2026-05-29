# SPEC-044: Integrated HSP Processing

**Crate**: `arkdown-pack`
**Priority**: P1
**Blocked by**: SPEC-012 (packager)
**Blocks**: —

---

## Summary

When a HAP depends on a remote HSP that is marked `isIntegratedHsp: true`, the HSP must be
embedded directly inside the HAP archive rather than deployed as a separate on-device package.
This is the "integrated HSP" (集成态HSP) feature introduced in API 11.

hvigor implements this via `ProcessIntegratedHsp` (in
`hvigor-ohos-plugin/src/tasks/process-integrated-hsp.js`) which calls the Java `packageTool`
with `--mode packageNormalize`. The normalize step patches the remote HSP's `pack.info` to
replace its original `bundleName` and `versionCode` with the host HAP's values, then embeds
the normalized HSP into the HAP archive.

This SPEC ports that logic to pure Rust in `arkdown-pack`.

---

## Background: hvigor `packageNormalize`

The Java `packageTool --mode packageNormalize` command:

1. Opens the input `.hsp` zip.
2. Reads `pack.info` from it.
3. Patches `summary.app.bundleName` and `summary.app.version.code` with the host values.
4. Writes the patched HSP to the output directory as `<hspDirName>/<hspFileName>`.

The host values come from `AppScope/app.json5` (`bundleName`, `versionCode`).

---

## Integrated HSP Archive Layout

A HAP that integrates remote HSPs embeds them under `libs/`:

```
entry.hap
├── module.json
├── pack.info
├── resources.index
├── ets/modules.abc
├── resources/
│   └── base/
└── libs/
    └── <hspDirName>/
        └── <hspFileName>.hsp    ← normalized HSP (patched pack.info)
```

The embedded HSP is a valid zip with its own `pack.info`, `ets/modules.abc`, and
`resources.index`. The runtime extracts and installs it alongside the HAP.

---

## API

```rust
/// Descriptor for a single remote HSP to be integrated into a HAP.
pub struct IntegratedHspEntry {
    /// Directory name used inside the HAP `libs/` tree (e.g. `"mylib@1.0.0"`).
    pub hsp_dir_name: String,
    /// Filename of the `.hsp` archive (e.g. `"mylib.hsp"`).
    pub hsp_file_name: String,
    /// Path to the pre-built `.hsp` file on disk.
    pub hsp_path: PathBuf,
}

/// Processes a list of remote HSPs for integration into a HAP.
///
/// For each entry:
///   1. Opens the source `.hsp` zip.
///   2. Reads and patches `pack.info` — replaces `summary.app.bundleName` and
///      `summary.app.version.code` with the host `bundle_name` / `version_code`.
///   3. Writes the normalized HSP to `output_dir/<hsp_dir_name>/<hsp_file_name>`.
///
/// Returns the list of output paths in the same order as `entries`.
pub fn normalize_integrated_hsps(
    entries: &[IntegratedHspEntry],
    bundle_name: &str,
    version_code: u64,
    output_dir: &Path,
) -> Result<Vec<PathBuf>, PackError>;
```

`Packager` gains an additional field on `PackInput`:

```rust
pub struct PackInput {
    // ... existing fields ...

    /// Remote HSPs to embed inside this HAP (integrated HSP feature, API >= 11).
    ///
    /// Each entry is a pre-normalized `.hsp` path produced by `normalize_integrated_hsps()`.
    /// The packager stores it at `libs/<hsp_dir_name>/<hsp_file_name>` inside the archive.
    pub integrated_hsps: Vec<IntegratedHspEntry>,
}
```

---

## Normalize Algorithm

```
fn normalize_integrated_hsp(src_hsp: &Path, bundle_name: &str, version_code: u64,
                              out_dir: &Path, hsp_dir_name: &str, hsp_file_name: &str)
    -> Result<PathBuf, PackError>

1. Open src_hsp as zip::ZipArchive.
2. Create output directory: out_dir / hsp_dir_name.
3. Create output file: out_dir / hsp_dir_name / hsp_file_name.
4. Open a ZipWriter on the output file.
5. For each entry in the source zip:
   a. If the entry name is "pack.info":
      - Read the bytes, parse as serde_json::Value.
      - Set value["summary"]["app"]["bundleName"] = bundle_name.
      - Set value["summary"]["app"]["version"]["code"] = version_code.
      - Serialize back to compact JSON + newline.
      - Write with Deflated compression.
   b. Otherwise:
      - Copy the entry verbatim (preserve compression method and raw bytes).
6. Finish the ZipWriter.
7. Return the output path.
```

---

## HAP Packager Integration

`pack_hap()` (and `pack_hsp()`) must embed integrated HSPs after all other entries:

```
for entry in &input.integrated_hsps {
    let dest = format!("libs/{}/{}", entry.hsp_dir_name, entry.hsp_file_name);
    zip_writer::add_file_stored(&mut writer, &dest, &entry.hsp_path)?;
}
```

The normalized HSP is stored (not deflated) inside the HAP because it is itself a zip archive.

---

## Minimum API Version

Integrated HSPs require API >= 11. The caller (CLI) is responsible for enforcing this;
`arkdown-pack` does not check the API version — it processes whatever `integrated_hsps` is
provided. An empty `integrated_hsps` vec is a no-op (backward compatible).

---

## Cache File

hvigor writes `integratedHspCache.json` to track which HSPs have been normalized. arkdown
does not implement incremental caching in this SPEC; the normalize step runs unconditionally.
Cache invalidation can be added later as part of SPEC-019.

---

## Test Requirements

1. `normalize_integrated_hsps()` with a single HSP entry:
   - Produces output at `<output_dir>/<hsp_dir_name>/<hsp_file_name>`.
   - The output is a valid zip.
   - `pack.info` in the output has `summary.app.bundleName` equal to the host `bundle_name`.
   - `pack.info` in the output has `summary.app.version.code` equal to `version_code`.
   - All other entries from the source HSP are present verbatim.

2. `normalize_integrated_hsps()` with an HSP that has no `pack.info`:
   - Returns `Err(PackError::MissingInput("pack.info"))`.

3. `pack_hap()` with one `integrated_hsps` entry:
   - The output HAP contains `libs/<hsp_dir_name>/<hsp_file_name>` stored (method 0).
   - All other standard HAP entries are still present.

4. `pack_hap()` with empty `integrated_hsps`:
   - Identical behavior to the pre-SPEC-044 baseline (no `libs/` directory created by
     the integrated HSP path; native libs still work independently).

---

## Design References

- `hvigor-ohos-plugin/src/tasks/process-integrated-hsp.js`
- `hvigor-ohos-plugin/src/utils/integrated-hsp-utils.js`
- `hvigor-ohos-plugin/src/const/build-directory-const.js`
  (`BuildDirConst.INTEGRATED_HSP = "integrated_hsp"`,
   `BuildArtifactConst.INTEGRATED_HSP_CACHE = "integratedHspCache.json"`)
- `meta/design/hvigor-gap-plan.md` — Phase 6 packaging cleanup
- `meta/tasks/SPEC-012-packager.md` — HAP/HSP/HAR base packager
