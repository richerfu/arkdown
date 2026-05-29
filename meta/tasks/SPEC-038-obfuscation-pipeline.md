# SPEC-038: Obfuscation Pipeline Wiring

**Crate**: `arkdown-obfuscate`, `arkdown-cli`
**Priority**: P1 — required for release builds
**Blocked by**: SPEC-020 (obfuscation crate), SPEC-009 (ABC compiler)
**Blocks**: SPEC-025 (ABC incremental cache — needs obfuscation hash in compileEnv)

---

## Summary

Wire the existing `arkdown-obfuscate` crate into the CLI build pipeline between rolldown
bundling and ABC compilation. Read obfuscation rules from
`build-profile.json5#buildOption.arkOptions.obfuscation`. Compute `obfuscationHash` for
cache invalidation. Emit a consumer obfuscation config file for HAR publishing.

The `arkdown-obfuscate` crate already has `transform.rs`, `rename.rs`, `rules.rs`, and
`minifier.rs`. This SPEC covers the plumbing that activates them.

---

## Background: What hvigor Does

From `ark-compile.js`:

```javascript
async beforeAlwaysAction() {
    this.obfuscationOptions = await this.getObfuscationOptions();
    if (this.obfuscationOptions) {
        this.obfuscationHash = this.computeObfuscationFileHash(this.obfuscationOptions);
    }
}

async getObfuscationOptions() {
    const buildOption = this.targetService.getBuildOption();
    // Obfuscation is ONLY for release builds
    if (buildOption.debuggable) return undefined;
    const obfuscation = buildOption.arkOptions?.obfuscation;
    // Resolve rule files, consumer rules, HAR/HSP dependency rules
    return resolveObfuscationOptions(this.targetService, obfuscation, cacheDir);
}

computeObfuscationFileHash(obfuscationOptions) {
    const files = [
        ...selfConfig.ruleOptions.rules,
        ...selfConfig.consumerRules,
        ...dependencies.hars,
        ...dependencies.hsps,
        // library rule files
    ];
    const hash = crypto.createHash('sha256');
    for (const f of files) {
        if (fse.existsSync(f)) hash.update(fse.readFileSync(f));
    }
    return hash.digest('hex');
}
```

From `arkts-pack.js::generateConfig()`:

```javascript
// compileEnv includes obfuscation state
function getCompileEnv(e, t) {
    return [
        e.compileSdkVersion, e.compatibleSdkVersion, e.runtimeOS,
        e.etsLoaderPath,
        !!e.obfuscationOptions?.selfConfig?.ruleOptions?.enable,  // ← obfuscation flag
        t.moduleType,
        path.resolve(t.cachePath, MSGPACK_COMPILE_CACHE_DIR_NAME),
    ].join("#");
}
```

The obfuscation pass runs between rolldown output and `es2abc`:

```javascript
// In doTaskAction() after rolldown:
if (obfuscationOptions) {
    await fse.ensureDir(obfuscationOptions.obfuscationCacheDir);
    await fse.ensureDir(path.dirname(obfuscationOptions.exportRulePath));
}
// Then submitArkCompileWork() which runs es2abc
```

Consumer obfuscation config (`exportRulePath`) is written by the SDK obfuscation
plugin for HAR publishing so downstream consumers can merge the rules.

---

## Scope

### 1. Build-Profile Obfuscation Config Parsing

Extend `arkdown-config` to parse the obfuscation section:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObfuscationConfig {
    pub rule_options: Option<RuleOptions>,
    pub consumer_rules: Option<Vec<PathBuf>>,
}

#[derive(Debug, Deserialize)]
pub struct RuleOptions {
    pub enable: Option<bool>,
    pub rules: Option<Vec<PathBuf>>,
}
```

Read from `build-profile.json5#buildOption.arkOptions.obfuscation`.

### 2. `should_obfuscate()` Check

```rust
/// Returns true only for release builds with obfuscation explicitly enabled.
pub fn should_obfuscate(build_mode: BuildMode, config: Option<&ObfuscationConfig>) -> bool {
    if build_mode != BuildMode::Release { return false; }
    config
        .and_then(|c| c.rule_options.as_ref())
        .and_then(|r| r.enable)
        .unwrap_or(false)
}
```

### 3. `obfuscationHash` Computation

```rust
/// Compute SHA-256 over all rule file contents (same as hvigor's computeObfuscationFileHash).
pub fn compute_obfuscation_hash(
    rule_files: &[PathBuf],
    consumer_rules: &[PathBuf],
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for path in rule_files.iter().chain(consumer_rules.iter()) {
        if let Ok(content) = std::fs::read(path) {
            hasher.update(&content);
        }
    }
    hex::encode(hasher.finalize())
}
```

### 4. CLI Pipeline Integration

In `crates/arkdown-cli/src/build.rs`, after rolldown bundling:

```rust
let obfuscation_config = config.build_profile.build_option
    .ark_options.as_ref()
    .and_then(|a| a.obfuscation.as_ref());

if should_obfuscate(config.build_mode, obfuscation_config) {
    let rule_files = obfuscation_config
        .unwrap().rule_options.as_ref()
        .and_then(|r| r.rules.as_ref())
        .cloned()
        .unwrap_or_default();
    let consumer_rules = obfuscation_config
        .unwrap().consumer_rules.as_ref()
        .cloned()
        .unwrap_or_default();

    let obfuscation_hash = compute_obfuscation_hash(&rule_files, &consumer_rules);

    let obfuscator = Obfuscator::new(ObfuscationOptions {
        rule_files,
        cache_dir: config.cache_dir.join("obfuscation"),
        export_rule_path: Some(config.intermediates_dir.join("consumer-obfuscation-rules.txt")),
    })?;

    // Obfuscate all .js chunks in loader_out/
    obfuscator.obfuscate_dir(&bundle_output_dir)?;

    // Wire obfuscation_hash into compileEnv for ABC cache invalidation
    abc_options.compile_env = Some(compute_compile_env_with_obfuscation(
        &config, &obfuscation_hash
    ));
} else {
    abc_options.compile_env = Some(compute_compile_env(&config));
}
```

### 5. Consumer Obfuscation Config for HAR

When building a HAR with obfuscation enabled, write the consumer rules to
`<intermediates>/consumer-obfuscation-rules.txt`. This file is included in the HAR
archive so downstream consumers can merge the obfuscation rules.

```rust
/// Write merged consumer obfuscation rules for HAR publishing.
pub fn write_consumer_obfuscation_config(
    consumer_rules: &[PathBuf],
    output_path: &Path,
) -> std::io::Result<()>;
```

### 6. Cache Invalidation on Obfuscation Change

From `ark-compile.js::beforeTask()`:

```javascript
const cachedEnable = this.getIncrementalCacheItem(OBFUSCATION_ENABLE);
if (String(cachedEnable) !== String(this.getObfuscationEnable())) {
    await this.cleanCache(); // wipe entire ABC cache
}
const cachedHash = this.getIncrementalCacheItem(OBFUSCATION_FILES_HASH);
if (cachedHash !== this.obfuscationHash) {
    await this.cleanCache();
}
```

In arkdown: persist `{ obfuscation_enable, obfuscation_hash }` to
`<cache_dir>/TaskCompileArkTS.cache` (JSON). On next build, compare and wipe the ABC
cache dir if either value changed.

---

## Files to Change

```
crates/arkdown-config/src/
└── lib.rs              — add ObfuscationConfig, RuleOptions to build-profile parsing

crates/arkdown-obfuscate/src/
└── lib.rs              — add compute_obfuscation_hash(), write_consumer_obfuscation_config()

crates/arkdown-cli/src/
└── build.rs            — wire obfuscation between rolldown output and ABC compile;
                          persist/check TaskCompileArkTS.cache
```

---

## Test Plan

- `should_obfuscate(Debug, Some(enable: true))` returns `false`.
- `should_obfuscate(Release, None)` returns `false`.
- `should_obfuscate(Release, Some(enable: true))` returns `true`.
- `compute_obfuscation_hash` is deterministic for the same rule files.
- `compute_obfuscation_hash` changes when any rule file content changes.
- Release build with `enable: true`: `.js` chunks in `loader_out/` are obfuscated before
  ABC compile; ABC output is smaller than unobfuscated equivalent.
- Debug build: obfuscation is never called; `.js` chunks are passed to ABC unmodified.
- `TaskCompileArkTS.cache` is written after each release build.
- Changing `enable` from `true` to `false` between builds causes ABC cache wipe.
- Changing rule file content causes ABC cache wipe.
- HAR build with `consumerRules`: `consumer-obfuscation-rules.txt` is written to
  intermediates dir.

---

## Upstream References

- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/src/tasks/ark-compile.js`
  — `getObfuscationOptions()`, `computeObfuscationFileHash()`, `beforeTask()`,
  `checkAndCleanCache()`
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/arkts-pack.js`
  — `getCompileEnv()` obfuscation flag inclusion
- `meta/tasks/SPEC-020-obfuscation.md` — existing `arkdown-obfuscate` crate API
- `meta/tasks/SPEC-019-cache.md` — cache invalidation ordering
