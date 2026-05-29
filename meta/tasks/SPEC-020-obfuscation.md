# SPEC-020: Obfuscation Pipeline

**Crate**: `arkdown-obfuscate`
**Priority**: P1 — required for release builds with obfuscation enabled
**Blocked by**: SPEC-009 (abc driver)
**Blocks**: —
**Status**: Implemented (2026-05-29)

---

## Summary

Pure Rust JS obfuscation and minification using `oxc_parser` + `oxc_minifier`.
Replaces the previous approach of invoking a closed-source SDK binary as a subprocess.

The crate is inserted between bundler output and `arkdown-abc` compilation. Two passes run:

1. **Identifier renaming** (when `-enable-obfuscation` is set): parses each bundled JS
   file with `oxc_parser`, collects top-level binding names (`var`/`let`/`const`,
   `function`, `class`), generates short replacement names (a, b, …, z, aa, …), and
   rewrites all whole-word occurrences in the source text using descending-offset splice
   (so earlier offsets are not invalidated by shorter replacements).  Property accesses
   (`.name`) are excluded from renaming.

2. **Minification** (when `-compact` is set): runs `oxc_minifier` with full compress
   (`CompressOptions::smallest()`: dead-code elimination, constant folding, variable
   joining) and `MangleOptions` for identifier compression, then re-emits via
   `CodegenOptions::minify()` (whitespace/comment removal).

OHOS rule files (the same `.txt` format used by hvigor) are parsed natively and control
which names are kept, which categories are obfuscated, and whether `console.log` calls
are stripped.

---

## When Activated

Only in release builds with obfuscation configured:

```json5
// build-profile.json5
buildOption: {
  arkOptions: {
    obfuscation: {
      ruleOptions: { enable: true, rules: ["./obfuscation-rules.txt"] },
      consumerRules: []
    }
  }
}
```

Obfuscation is always disabled in debug builds. From hvigor:

```javascript
if (buildOption.debuggable) return; // obfuscation is only for release
```

---

## Rule File Format

```text
# comment
-enable-obfuscation
-enable-property-obfuscation
-enable-toplevel-obfuscation
-compact
-remove-log

-keep-property-name
propertyToKeep
anotherProperty

-keep-global-name
GlobalClass
globalFunction
```

Directives are parsed by `crate::rules::parse_rules_str`. Multiple rule files are merged
with logical-OR for flags and set-union for keep-name lists.

---

## Pipeline Integration

Insert between bundler output and es2abc:

```
arkdown bundle → bundled .js files
                           │
                    [if obfuscation.enable && release]
                           │
                    arkdown-obfuscate (pure Rust, oxc_parser)
                           │
                    obfuscated .js files
                           │
                    arkdown-abc (Rust) → .abc
```

---

## Cache Invalidation

Compute a FNV-1a hash of all obfuscation rule file contents with `compute_rule_hash`.
If the hash changes between builds, discard the es2abc cache for affected modules
(same logic as hvigor's `ArkCompile.beforeTask()`).

---

## Acceptance Criteria

- Release build with `ruleOptions.enable: true` runs obfuscation before es2abc
- Debug build never runs obfuscation
- Obfuscation cache invalidated when rule files change
- No subprocess invocation — entirely in-process Rust

---

## Design References

- `meta/design/abc-pipeline.md` — obfuscation section (Section 10)
- `meta/design/hvigorw-analysis.md` — `getObfuscationOptions`, `computeObfuscationFileHash`

---

## Rolldown Alignment

| arkdown concept                                   | rolldown equivalent                              | note                                                                                                                        |
| ------------------------------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| `oxc_parser` + span-based renaming                | rolldown `minify` option (built-in oxc minifier) | rolldown uses the full oxc minifier; arkdown uses a targeted identifier-rename pass controlled by OHOS rule files           |
| Insert between bundler output and ABC compilation | no rolldown equivalent                           | rolldown has no compilation stage after JS emit; this step is OHOS-specific                                                 |
| `ObfuscationRules::keep_global_names`             | rolldown `minify` configuration                  | Different mechanism: OHOS obfuscation uses rule files for identifier renaming; rolldown's minifier uses built-in heuristics |
| Obfuscation rule file hash → cache invalidation   | rolldown watches `rollup.config.js` for changes  | Both invalidate caches when configuration changes; different mechanisms                                                     |
| `BuildMode::Debug` → skip obfuscation             | rolldown dev mode (watch mode)                   | Both skip obfuscation/minification in development builds                                                                    |

---

## Public API

```rust
/// Configuration for a single obfuscation pass.
pub struct ObfuscationOptions {
    pub rule_files: Vec<PathBuf>,
    pub cache_dir: PathBuf,
    pub export_rule_path: Option<PathBuf>,
}

pub struct Obfuscator { /* opaque */ }

impl Obfuscator {
    /// Parse rule files and build the obfuscator.  Returns
    /// `ObfuscationError::RuleFileNotFound` if any rule file is absent.
    pub fn new(options: ObfuscationOptions) -> Result<Self, ObfuscationError>;

    /// Obfuscate JS source in memory.
    pub fn obfuscate_source(&self, source: &str, filename: &str)
        -> Result<ObfuscationOutput, ObfuscationError>;

    /// Obfuscate a single .js file (read input, write output).
    pub fn obfuscate_file(&self, input: &Path, output: &Path)
        -> Result<PathBuf, ObfuscationError>;

    /// Obfuscate all .js files up to 2 levels deep in a directory, in-place.
    pub fn obfuscate_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, ObfuscationError>;

    /// FNV-1a hash of all rule file contents.  Used for ABC cache invalidation.
    pub fn rule_hash(&self) -> u64;
}

#[derive(Debug, thiserror::Error)]
pub enum ObfuscationError {
    Io(#[from] std::io::Error),
    RuleFileNotFound(PathBuf),
    ParseError { file: String, message: String },
    CodegenError(String),
}

pub fn compute_rule_hash(rule_files: &[PathBuf]) -> u64;
pub fn should_obfuscate(build_mode: &BuildMode, config: Option<&ObfuscationConfig>) -> bool;
pub fn parse_rules_str(content: &str) -> ObfuscationRules;
pub fn parse_rules_file(path: &Path) -> std::io::Result<ObfuscationRules>;
pub fn merge_rules(base: ObfuscationRules, extra: ObfuscationRules) -> ObfuscationRules;
```

---

## Test Requirements

- `BuildMode::Debug` + any config: `should_obfuscate` returns `false`
- `BuildMode::Release` + `enable_obfuscation: false`: `should_obfuscate` returns `false`
- `BuildMode::Release` + `enable_obfuscation: true`: `should_obfuscate` returns `true`
- `compute_rule_hash` is deterministic for unchanged rule files
- `compute_rule_hash` changes when rule file content changes
- Rule parser correctly sets all enable flags and keep-name sets
- Comments and blank lines in rule files are ignored
- `-disable-*` directive overrides earlier `-enable-*`
- `Obfuscator::new` with no rule files produces an obfuscator that passes source through
- `obfuscate_source` renames top-level `var` declarations when `-enable-obfuscation` is set
- `obfuscate_source` preserves names in `-keep-global-name`
- `-remove-log` strips `console.log(...)` lines
- `obfuscate_file` reads input, writes obfuscated output, returns output path
- `ObfuscationError::RuleFileNotFound` returned when rule file is absent
- Top-level `function` declarations are renamed
- Property accesses (`.propName`) are NOT renamed even when the identifier appears in bindings
