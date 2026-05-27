# SPEC-020: Obfuscation Pipeline

**Crate**: `arkdown-obfuscate`
**Priority**: P1 — required for release builds with obfuscation enabled
**Blocked by**: SPEC-009 (abc driver)
**Blocks**: —

---

## Summary

Invokes the OHOS obfuscation tool (part of the SDK) on the bundled JS output before es2abc
compilation. Replaces hvigor's obfuscation step in `ArkCompile.getObfuscationOptions()`.

The obfuscation tool is a closed-source SDK binary — arkdown invokes it as a subprocess
between the bundler output and the es2abc invocation.

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

## Obfuscation Config Types

```rust
pub struct ObfuscationOptions {
    pub self_config: ObfuscationSelfConfig,
    pub dependencies: ObfuscationDependencies,
    pub obfuscation_cache_dir: PathBuf,
    pub export_rule_path: PathBuf,
}

pub struct ObfuscationSelfConfig {
    pub rule_options: RuleOptions,
    pub consumer_rules: Vec<PathBuf>,
    pub lib_dir: PathBuf,
}

pub struct ObfuscationDependencies {
    pub hars: Vec<PathBuf>,
    pub hsps: Vec<PathBuf>,
    pub libraries: Vec<LibraryObfuscation>,
}
```

---

## Pipeline Integration

Insert between bundler output and es2abc:

```
arkdown bundle → bundled .js files
                           │
                    [if obfuscation.enable && release]
                           │
                    obfuscation tool subprocess
                           │
                    obfuscated .js files
                           │
                    arkdown-abc (Rust) → .abc
```

---

## Cache Invalidation

Compute a hash of all obfuscation rule files. If the hash changes between builds, discard
the es2abc cache for affected modules (same logic as in hvigor's `ArkCompile.beforeTask()`).

---

## Acceptance Criteria

- Release build with `ruleOptions.enable: true` runs obfuscation tool before es2abc
- Debug build never runs obfuscation tool
- Obfuscation cache invalidated when rule files change

---

## Design References

- `meta/design/abc-pipeline.md` — obfuscation section (Section 10)
- `meta/design/hvigorw-analysis.md` — `getObfuscationOptions`, `computeObfuscationFileHash`

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
| --- | --- | --- |
| Obfuscation tool subprocess invocation | rolldown `minify` option (built-in oxc minifier) | In rolldown, minification is a built-in stage using oxc's minifier; in arkdown, OHOS obfuscation is a separate closed-source tool invoked as a subprocess — not the same as minification |
| Insert between bundler output and ABC compilation | no rolldown equivalent | rolldown has no compilation stage after JS emit; this step is OHOS-specific |
| `ObfuscationOptions::self_config.rule_options` | rolldown `minify` configuration | Different mechanism: OHOS obfuscation uses rule files for identifier renaming; rolldown's minifier uses built-in heuristics |
| Obfuscation rule file hash → cache invalidation | rolldown watches `rollup.config.js` for changes | Both invalidate caches when configuration changes; different mechanisms |
| `is_debug: true` → skip obfuscation | rolldown dev mode (watch mode) | Both skip obfuscation/minification in development builds |

**Key divergence:** The OHOS obfuscation tool is closed-source and must be invoked as a subprocess. Unlike the Rust-ported tools (es2abc, restool, ets-loader), there is no open-source reference to port. This crate manages subprocess I/O and cache invalidation around the tool.

---

## Public API

```rust
pub struct ObfuscationRunner {
    options: ObfuscationOptions,
    rule_file_hash: u64,
}

impl ObfuscationRunner {
    pub fn new(options: ObfuscationOptions) -> Result<Self, ObfuscationError>;
    
    /// Run obfuscation on bundled JS files. Only call in release builds.
    /// Returns paths to the obfuscated JS files (replacing input files in-place or to a new dir).
    pub fn run(&self, js_files: &[PathBuf]) -> Result<Vec<PathBuf>, ObfuscationError>;
    
    /// Hash of all rule files. Used to detect configuration changes for cache invalidation.
    pub fn rule_hash(&self) -> u64;
}
```

---

## Test Requirements

- Debug build (`BuildMode::Debug`): `ObfuscationRunner::run()` is never called — verified by checking the bundler output is unchanged
- Release build with `ruleOptions.enable: false`: `ObfuscationRunner::run()` is never called
- Release build with `ruleOptions.enable: true`: subprocess is invoked; output files are written
- `rule_hash()` changes when a rule file content changes
- `rule_hash()` is stable when rule files are unchanged
- Cache invalidation: when `rule_hash()` changes between builds, `should_invalidate_cache` returns `true` for all ABC-compiled modules
- `ObfuscationError::ToolNotFound` returned when obfuscation binary is absent from SDK (not panic)
- `ObfuscationError::SubprocessFailed { exit_code, stderr }` returned when tool exits non-zero
