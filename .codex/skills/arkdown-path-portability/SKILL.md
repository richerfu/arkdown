---
name: arkdown-path-portability
description: Mandatory arkdown cross-platform path and artifact-path rules. Use when Codex edits, reviews, audits, or plans filesystem path handling, artifact path serialization, resolver paths, module specifiers, OHM URLs, path normalization, cache keys, package path classification, Windows/Unix portability, non-UTF-8 display, `oh_modules` classification, or code containing starts_with('/'), contains("oh_modules"), to_string_lossy, trim_start_matches on paths, slash replacement, or PathBuf/String path conversions.
---

# Arkdown Path Portability

## Required Types

- Filesystem paths use `Path`, `PathBuf`, `AbsPath`, or `RelPath`.
- Artifact paths use `ArtifactPath`.
- Module import/export text uses `ModuleSpecifier`.
- Runtime OHM URLs use `OhmUrl`.
- Diagnostics use `PathDisplay`.
- Do not use `String` as a generic path identity.

## Arkdown Path Crate

`arkdown-path` owns:

- `AbsPath`
- `RelPath`
- `ArtifactPath`
- `ModuleSpecifier`
- `OhmUrl`
- `PathDisplay`
- lexical normalization
- path component classification
- artifact slash serialization
- case-exact checks

No other crate owns slash conversion or path classification policy.

## Forbidden Patterns

- `path_str.starts_with('/')`
- `path_str.contains("oh_modules")`
- inline `.replace('\\', "/")`
- `.to_string_lossy()` for identity, cache keys, package matching, resolver
  matching, or dependency graph keys
- `trim_start_matches("./")` on filesystem paths
- repeated extension stripping with string suffix trimming
- `format!("{}/...", path)` for filesystem paths
- treating artifact slash paths and real filesystem paths as the same type

## Normalization Policy

- Existing input paths are canonicalized only inside `PathResolver` or startup
  builders.
- Output paths that do not exist are lexically normalized.
- Symlink behavior is explicit at resolver identity boundaries.
- Artifact output preserves configured roots where hvigor compatibility requires
  it.
- Cache identity uses normalized absolute paths plus content hashes.

## Artifact Serialization

Only `ArtifactPathSerializer` produces slash-separated paths for:

- loader JSON
- `pkgContextInfo.json`
- source maps
- importee maps
- bytecode HAR metadata
- resource table references
- zip entry names
- hvigor parity intermediates

Every other crate keeps `PathBuf`, `AbsPath`, `RelPath`, or typed specifier/URL
values.

## Required Replacements

| Old pattern | Replacement |
| --- | --- |
| `path_str.starts_with('/')` | `Path::new(path_str).is_absolute()` |
| `contains("oh_modules")` | component-based `PathResolver::classify_modules_path` |
| `trim_start_matches("./")` | parse as specifier or `RelPath::parse` |
| `trim_end_matches(".ets")` | path/module-id helper |
| string path `format!` | `PathBuf::join` |
| inline slash replacement | `ArtifactPathSerializer` |
| ad hoc canonicalize fallback | `PathResolver::abs_existing_or_lexical` |

## Tests

- Path serializer tests run on every platform.
- Windows-drive and UNC behavior has `#[cfg(windows)]` tests.
- Non-UTF-8 display behavior is tested where the platform supports it.
- `oh_modules` classification uses components, not substring matching.
- Case-exact checks are directory-entry based, not full-path lowercasing.
