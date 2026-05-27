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
