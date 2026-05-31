# SPEC-011: Resource Compiler (Rust)

**Crate**: `arkdown-resource`
**Priority**: P0
**Blocked by**: SPEC-002
**Blocks**: SPEC-012

---

## Summary

Implement the OHOS resource compilation pipeline in pure Rust. This is a Rust port of
`restool` — the resource compiler that transforms `src/main/resources/` into binary
`resources.index` and compiled resource tables.

**Reference implementation**: `https://github.com/openharmony/developtools_global_resource_tool`

No subprocess invocation. All resource compilation logic lives in this crate.

---

## Background: What restool Does

From the OpenHarmony `developtools_global_resource_tool` source, the compilation pipeline:

1. **Scan** — walk `resources/` directory tree, categorize files by type and qualifier
2. **Parse** — parse `element/string.json`, `element/color.json`, etc. into resource entries
3. **Assign IDs** — assign integer resource IDs (starting from `0x01000000` for app resources)
4. **Build index** — serialize all resource entries into `resources.index` binary format
5. **Emit table** — write `ResourceTable.txt` (human-readable ID map for build tools)
6. **Copy media** — copy media files (images, audio) to output with consistent naming

---

## Resource Directory Structure

```
src/main/resources/
├── base/                         ← default (fallback) resources
│   ├── element/
│   │   ├── string.json           ← string resources
│   │   ├── color.json            ← color values
│   │   ├── float.json            ← dimension/float values
│   │   └── integer.json
│   ├── media/
│   │   ├── app_icon.png
│   │   └── background.jpg
│   └── profile/
│       └── main_pages.json       ← profile JSON (not compiled, just bundled)
├── en_US/                        ← locale qualifier
│   └── element/
│       └── string.json
└── zh_CN/
    └── element/
        └── string.json
```

---

## resources.index Binary Format

From `developtools_restool` source. The format is a custom binary:

```
Header:
  magic: u32 = 0xF000 (verify in restool source)
  version: u32
  resource_count: u32
  ...

ResourceEntry:
  id: u32                 ← numeric resource ID (e.g. 0x01000001)
  type: ResourceType      ← STRING, COLOR, FLOAT, MEDIA, etc.
  name: &str              ← resource name (e.g. "app_name")
  values: Vec<Value>      ← one per qualifier (base + locales)

Value:
  qualifier: &str         ← "" for base, "en_US" for en_US/, etc.
  data: ResourceData
```

Read `developtools_restool/src/` to understand the exact binary layout.

---

## Resource Types

| Type      | Source file                 | Example                                                     |
| --------- | --------------------------- | ----------------------------------------------------------- |
| `String`  | `element/string.json`       | `{ "string": [{ "name": "app_name", "value": "My App" }] }` |
| `Color`   | `element/color.json`        | `{ "color": [{ "name": "primary", "value": "#FF0000" }] }`  |
| `Float`   | `element/float.json`        | dimensions, scaling factors                                 |
| `Integer` | `element/integer.json`      | integer constants                                           |
| `Media`   | `media/*.{png,jpg,svg,...}` | copied to output                                            |
| `Profile` | `profile/*.json`            | bundled as-is (not compiled to binary)                      |

---

## Resource ID Assignment

IDs follow OHOS conventions from `developtools_restool`:

- App resources start at `0x01000000`
- System resources start at `0x08000000` (not our concern)
- IDs are assigned sequentially per type within a module

The `ResourceTable.txt` output format:

```
ResourceTable
  entry: 0x01000000  string  app_name
  entry: 0x01000001  string  module_desc
  ...
```

---

## API

```rust
pub struct ResourceCompiler {
    config: ResourceCompileConfig,
}

pub struct ResourceCompileConfig {
    pub package_name: String,
    pub module_name: String,
    pub device_types: Vec<String>,
    pub is_debug: bool,
}

pub struct ResourceCompileInput {
    pub resources_dirs: Vec<PathBuf>,   // multiple dirs merged (HAR deps + self)
    pub output_dir: PathBuf,
    pub profile_inputs: Vec<ProfileInput>,   // router map, startup config, etc.
}

pub struct ResourceCompileOutput {
    pub resources_index: PathBuf,       // output resources.index
    pub resource_table: PathBuf,        // ResourceTable.txt
    pub profiles: Vec<(String, PathBuf)>, // ("routerMap", path), etc.
}

impl ResourceCompiler {
    pub fn compile(&self, input: ResourceCompileInput)
        -> Result<ResourceCompileOutput, ResourceError>;
}
```

---

## Profile Handling

Profile JSON files (router map, startup config, share config) are NOT compiled into the
binary `resources.index`. Instead, they are:

1. Written to `resources/base/profile/<name>.json` in the output dir
2. Referenced from `module.json` via `"$profile:<name>"` syntax
3. Packaged into the HAP alongside `resources.index`

The `ResourceCompiler` handles writing these profile JSON files to the output.

---

## Files to Create

```
crates/arkdown-resource/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── compiler.rs         (ResourceCompiler: orchestration)
    ├── scan.rs             (directory walker, qualifier parsing)
    ├── element.rs          (string/color/float/integer JSON parsing)
    ├── media.rs            (media file copy + ID assignment)
    ├── index.rs            (resources.index binary format writer)
    ├── table.rs            (ResourceTable.txt text writer)
    └── profile.rs          (profile JSON writing)
```

---

## Acceptance Criteria

- `compile()` on `hmosworld` `entry/src/main/resources/` produces a valid `resources.index`
- Produced `resources.index` is accepted by HAP packager (same format as restool output)
- String resources from `base/element/string.json` and locale overrides both present in index
- Media files copied to output with correct names
- `ResourceTable.txt` lists all assigned resource IDs

---

## Design References

- `meta/design/pipeline.md` — Phase 10 (Resource Compilation)
- `meta/design/hvigorw-analysis.md` — CompileResource task context
- OpenHarmony source: `https://github.com/openharmony/developtools_global_resource_tool`

---

## Rolldown Alignment

`arkdown-resource` has no rolldown equivalent — resource compilation is entirely OHOS-specific. Standard JS bundlers do not process binary resource indexes. The closest analogue in the JS ecosystem is a Vite/webpack asset plugin, but the OHOS `resources.index` binary format and ID assignment system are unique to OpenHarmony.

| arkdown concept                                    | closest analogue                               | note                                                                                              |
| -------------------------------------------------- | ---------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `ResourceCompiler::compile()`                      | Vite `transformAssets` / webpack `file-loader` | Conceptually similar (process non-JS assets) but the binary output format is completely different |
| Resource ID assignment (`0x01000000+`)             | CSS Modules class name hashing                 | Different: OHOS uses sequential integer IDs; CSS Modules use content hashes                       |
| `resources.index` binary format                    | no equivalent in rolldown/rollup               | Unique OHOS binary format; must be ported from `developtools_restool` source                      |
| Qualifier-based locale merging (`base/`, `en_US/`) | webpack `i18n-loader`                          | Functionally similar but different format                                                         |
| Profile JSON passthrough (`profile/*.json`)        | webpack `copy-webpack-plugin`                  | Profile JSON files are bundled as-is, not compiled                                                |

---

## Test Requirements

- `compile()` on a minimal `resources/base/element/string.json` with two string entries produces `resources.index` with both entries and sequential IDs starting at `0x01000000`
- `resources.index` magic bytes match the expected value from `developtools_restool` source
- String resource from `base/` and locale override from `en_US/` both appear in the index under the same resource ID but different qualifier entries
- Color resource from `element/color.json`: hex value `"#FF0000"` is correctly parsed and stored
- Media file (`media/app_icon.png`) is copied to output directory with preserved filename
- Profile JSON (`profile/main_pages.json`) is copied to `resources/base/profile/` without modification (not compiled into binary index)
- Resource IDs are stable across rebuilds when the resource set is unchanged (deterministic assignment)
- Missing `resources/` directory returns `Err` (not panic)
- Malformed `element/string.json` (invalid JSON) returns `Err(ResourceError::ParseError)`
- `ResourceTable.txt` lists all assigned IDs in `entry: 0x01000000 string app_name` format
- HAR dependency resources merged with entry resources: IDs do not collide between HAR and entry resource sets
- `compile()` result `resources_index` path exists on disk after successful call
