# SPEC-037: ArkData Compile and ETS Standalone Checker

**Crates**: `arkdown-abc` (ArkData), `arkdown-cli` (standalone checker)
**Priority**: P2 (advanced — ArkData for data-driven apps; checker for lint-only CI mode)
**Blocked by**: SPEC-009 (ArkData); SPEC-004a (ETS checker)
**Upstream refs**:
- `ark-compile.js::arkDataCompile()` — ArkData separate compiler invocation
- `etsStandaloneChecker` from SDK — `load-sdk-functions.js`
- hvigor gap analysis: gaps H13, M5

---

## Background

Two advanced compilation features are not yet implemented:

1. **ArkData compile**: ArkData is a data-binding framework for OHOS that uses `.ets`
   files with `@ObservedV2` and `@Trace` decorators to define reactive data models. It
   requires a separate compilation pass from the main module because ArkData source files
   are compiled with different options.

2. **ETS Standalone Checker**: A lint-only mode that runs TypeScript type checking on
   `.ets` files without producing any output artifacts. Used in CI pipelines for
   incremental type-check-only builds.

---

## Part 1: ArkData Compile (H13)

### hvigor Behavior

```javascript
// ark-compile.js::arkDataCompile()
if (this.needArkDataCompile) {
    const arkDataConfig = {
        ...baseConfig,
        arkDataCompile: "true",
        cachePath: taskTempDir + "_arkdata",
        aceBuildJson: pathInfo.getArkDataLoaderJsonPath(),
        sourceFiles: this.arkDataSourceFiles,
    };
    await submitArkCompileWork(...arkDataConfig...);
}
```

### What is ArkData

ArkData source files are `.ets` files that use `@ObservedV2` and `@Trace` decorators (the
new reactive data model, distinct from the older `@Observed`/`@ObjectLink` pattern). They
are compiled with:
- A separate `loader.json` (`arkdata_loader.json`) that lists only ArkData source files
- A separate cache directory (`<taskTempDir>_arkdata/`)
- The `arkDataCompile: "true"` flag passed to `es2abc`

ArkData compilation produces a separate `.abc` file that is merged into the main `modules.abc`
via the `--merge-abc` flag.

### Detection

```rust
// crates/arkdown-abc/src/compiler.rs

pub struct AbcCompilerConfig {
    // ...existing fields...
    /// Whether this is an ArkData compile pass.
    pub ark_data_compile: bool,
}

/// Detect ArkData source files in the module.
/// Returns paths to .ets files using @ObservedV2 or @Trace decorators.
pub fn detect_ark_data_sources(
    src_path: &Path,
) -> Result<Vec<PathBuf>, AbcError> {
    let mut sources = Vec::new();
    for entry in walkdir::WalkDir::new(src_path) {
        let entry = entry?;
        if entry.path().extension().map_or(false, |e| e == "ets") {
            let content = std::fs::read_to_string(entry.path())?;
            if content.contains("@ObservedV2") || content.contains("@Trace") {
                sources.push(entry.path().to_owned());
            }
        }
    }
    Ok(sources)
}
```

### ArkData loader.json

The ArkData pass uses a separate `arkdata_loader.json` that lists only the ArkData source
files as compile entries:

```rust
// crates/arkdown-profile/src/loader_json.rs

impl LoaderJson {
    /// Generate a loader.json variant for the ArkData compile pass.
    /// Contains only the ArkData source files as compileEntry.
    pub fn for_ark_data_compile(&self, ark_data_sources: &[PathBuf]) -> Self {
        let mut arkdata_loader = self.clone();
        arkdata_loader.compile_entry = ark_data_sources
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        arkdata_loader
    }
}
```

### Compile Pass

```rust
// crates/arkdown-abc/src/compiler.rs

impl AbcCompiler {
    /// Run the ArkData compile pass (separate from main module compilation).
    /// Returns the path to the compiled ArkData .abc file.
    pub async fn compile_ark_data(
        &mut self,
        ark_data_sources: &[PathBuf],
        cache_dir: &Path,
    ) -> Result<PathBuf, AbcError> {
        let arkdata_cache = cache_dir.with_extension("_arkdata");
        let arkdata_loader = self.loader_json.for_ark_data_compile(ark_data_sources);
        let arkdata_config = AbcCompilerConfig {
            ark_data_compile: true,
            cache_path: Some(arkdata_cache),
            ..self.config.clone()
        };
        let mut compiler = AbcCompiler::new(arkdata_config, arkdata_loader);
        compiler.compile(AbcInput::from_sources(ark_data_sources)).await
            .map(|out| out.abc_path)
    }
}
```

### Merge into modules.abc

The ArkData `.abc` is merged into the main `modules.abc` using the `--merge-abc` pass:

```rust
// crates/arkdown-abc/src/merge.rs

/// Merge an ArkData .abc file into the main modules.abc.
pub fn merge_arkdata_abc(
    main_abc: &mut AbcBinary,
    arkdata_abc_path: &Path,
) -> Result<(), AbcError>;
```

### CLI Integration

```rust
// crates/arkdown-cli/src/main.rs

let ark_data_sources = detect_ark_data_sources(&module_src_path)?;
if !ark_data_sources.is_empty() {
    let arkdata_abc = abc_compiler.compile_ark_data(&ark_data_sources, &cache_dir).await?;
    merge_arkdata_abc(&mut main_abc, &arkdata_abc)?;
}
```

---

## Part 2: ETS Standalone Checker (M5)

### hvigor Behavior

```javascript
// load-sdk-functions.js
const etsStandaloneChecker = require(path.join(etsLoaderPath, 'standalone_checker.js'));
// In lint-only mode:
etsStandaloneChecker.check(sourceFiles, compilerOptions);
```

The standalone checker runs TypeScript type checking on `.ets` files without producing
output artifacts. It is invoked when `buildMode` is `"lint"` or via a separate
`arkdown check` CLI command.

### arkdown Implementation

Since `arkdown-abc` is a pure Rust compiler (no TypeScript checker), the ETS standalone
checker in arkdown is a best-effort type checking pass using the `ohos-rs/oxc` parser's
semantic analysis capabilities.

**Scope**: This is NOT a full TypeScript type checker (that would require porting
`ets_checker.js`). Instead, it provides:
- Syntax error detection (parse errors)
- Undefined variable detection (basic scope analysis)
- Decorator validation (unknown decorators on component structs)
- Import resolution errors (unresolvable `@ohos:*` or `@kit.*` imports)

Full semantic type checking (type mismatch, method signature errors) is out of scope for
this SPEC and would require SPEC-038 (etsChecker port).

### CLI Command

```
arkdown check [--project <path>] [--module <name>]
```

This command runs the standalone checker and exits with code 0 (no errors) or 1 (errors
found). No output artifacts are produced.

### Public API

```rust
// crates/arkdown-cli/src/commands/check.rs

pub struct CheckOptions {
    pub project_path: PathBuf,
    pub module_name: Option<String>,
    /// Output format: "text" (default) or "json"
    pub format: CheckOutputFormat,
}

pub enum CheckOutputFormat {
    Text,
    Json,
}

pub struct CheckDiagnostic {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
}

pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// Run standalone check on all .ets files in the module.
/// Returns list of diagnostics. Empty list = no errors.
pub async fn run_check(options: &CheckOptions) -> Result<Vec<CheckDiagnostic>, CheckError>;
```

### Checker Passes

```rust
// crates/arkdown-plugin-ets-transform/src/checker.rs

pub struct EtsChecker {
    resolver: Arc<OhosResolver>,
}

impl EtsChecker {
    /// Check a single .ets file. Returns diagnostics.
    pub fn check_file(
        &self,
        source: &str,
        file_path: &str,
    ) -> Vec<CheckDiagnostic> {
        let mut diagnostics = Vec::new();
        diagnostics.extend(self.check_syntax(source, file_path));
        diagnostics.extend(self.check_imports(source, file_path));
        diagnostics.extend(self.check_decorators(source, file_path));
        diagnostics
    }

    fn check_syntax(&self, source: &str, file: &str) -> Vec<CheckDiagnostic>;
    fn check_imports(&self, source: &str, file: &str) -> Vec<CheckDiagnostic>;
    fn check_decorators(&self, source: &str, file: &str) -> Vec<CheckDiagnostic>;
}
```

---

## Public API Summary

```rust
// crates/arkdown-abc/src/compiler.rs (extensions)
impl AbcCompiler {
    pub async fn compile_ark_data(
        &mut self,
        sources: &[PathBuf],
        cache_dir: &Path,
    ) -> Result<PathBuf, AbcError>;
}

pub fn detect_ark_data_sources(src_path: &Path) -> Result<Vec<PathBuf>, AbcError>;

// crates/arkdown-abc/src/merge.rs
pub fn merge_arkdata_abc(main_abc: &mut AbcBinary, arkdata_path: &Path) -> Result<(), AbcError>;

// crates/arkdown-cli/src/commands/check.rs
pub async fn run_check(options: &CheckOptions) -> Result<Vec<CheckDiagnostic>, CheckError>;
```

---

## Rolldown Alignment

These features have no rolldown equivalent.

| arkdown concept | hvigor equivalent | note |
|---|---|---|
| `compile_ark_data()` | `arkDataCompile()` in `ark-compile.js` | Separate es2abc invocation with `arkDataCompile: "true"` |
| `merge_arkdata_abc()` | `--merge-abc` flag to es2abc | Merge multiple .abc files into one |
| `run_check()` | `etsStandaloneChecker` from SDK | Lint-only mode; no output artifacts |

---

## Test Requirements

### ArkData Detection

- Create a `.ets` file with `@ObservedV2` decorator
- Assert: `detect_ark_data_sources()` returns the file path
- Create a `.ets` file with no `@ObservedV2` or `@Trace`
- Assert: `detect_ark_data_sources()` does NOT return the file path

### ArkData Compile Pass

- Build a module with ArkData source files
- Assert: `compile_ark_data()` produces a valid `.abc` file
- Assert: the ArkData `.abc` uses a separate cache directory (`<cache>_arkdata/`)
- Assert: `ark_data_compile: true` flag is set in the compiler config

### ArkData Merge

- Merge a valid ArkData `.abc` into a main `modules.abc`
- Assert: the merged `.abc` contains records from both inputs
- Assert: the merged `.abc` has valid Panda magic header (`b"PANDA\x00\x00\x00"`)

### ArkData — No Sources

- Build a module with no `@ObservedV2` or `@Trace` files
- Assert: `compile_ark_data()` is not called
- Assert: `modules.abc` is unchanged from the non-ArkData build

### ETS Checker — Syntax Error

- Run `arkdown check` on a `.ets` file with a syntax error (e.g., unclosed brace)
- Assert: exit code is 1
- Assert: diagnostic output contains the file name, line number, and error message

### ETS Checker — Unresolvable Import

- Run `arkdown check` on a file with `import { Foo } from '@ohos.nonexistent.module'`
- Assert: diagnostic output contains an import resolution error

### ETS Checker — Unknown Decorator

- Run `arkdown check` on a file with `@UnknownDecorator struct Foo {}`
- Assert: diagnostic output contains a warning about the unknown decorator

### ETS Checker — Clean File

- Run `arkdown check` on a syntactically valid `.ets` file with resolvable imports
- Assert: exit code is 0
- Assert: no diagnostics are emitted

### ETS Checker — JSON Output

- Run `arkdown check --format json` on a file with errors
- Assert: output is valid JSON
- Assert: JSON contains `[{ "file": "...", "line": N, "column": N, "severity": "error", "message": "..." }]`

### ETS Checker — No Artifacts

- Run `arkdown check` on a project
- Assert: no `.abc`, `.hap`, or `loader.json` files are created
- Assert: no `build/` directory modifications occur
