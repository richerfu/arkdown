//! Pure-Rust JS obfuscation for arkdown release builds.
//!
//! Replaces the closed-source SDK `obfuscation` subprocess previously invoked
//! between bundler output and `arkdown-abc` compilation.  The implementation
//! uses [`oxc_parser`] to parse each file and performs span-based identifier
//! renaming according to OHOS rule files.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use arkdown_obfuscate::{Obfuscator, ObfuscationOptions};
//! use std::path::PathBuf;
//!
//! let ob = Obfuscator::new(ObfuscationOptions {
//!     rule_files: vec![PathBuf::from("obfuscation-rules.txt")],
//!     cache_dir: PathBuf::from("build/.obf-cache"),
//!     export_rule_path: None,
//! })?;
//!
//! let out = ob.obfuscate_source("var longName = 1;", "entry.js")?;
//! assert!(!out.code.contains("longName"));
//! ```

pub(crate) mod minifier;
mod rename;
mod rules;
mod transform;

pub use rules::ObfuscationRules;
pub use transform::ObfuscationOutput;

use std::path::{Path, PathBuf};

use arkdown_core::{BuildMode, ObfuscationConfig};

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ObfuscationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Rule file not found: {0}")]
    RuleFileNotFound(PathBuf),
    #[error("Parse error in {file}: {message}")]
    ParseError { file: String, message: String },
    #[error("Codegen error: {0}")]
    CodegenError(String),
}

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for a single obfuscation pass.
#[derive(Debug, Clone)]
pub struct ObfuscationOptions {
    /// Rule files whose directives control obfuscation behaviour.
    pub rule_files: Vec<PathBuf>,
    /// Cache directory for obfuscation outputs (used by the caller for
    /// incremental build cache invalidation; not written to by this crate).
    pub cache_dir: PathBuf,
    /// Export rule path (consumer rules for HAR publishing).
    pub export_rule_path: Option<PathBuf>,
}

// ── Obfuscator ────────────────────────────────────────────────────────────────

/// Pure-Rust JS obfuscator.
///
/// Create once per build (rules are parsed at construction time) and reuse
/// across all files in the same build.
pub struct Obfuscator {
    rules: ObfuscationRules,
    rule_hash_value: u64,
}

impl Obfuscator {
    /// Create a new [`Obfuscator`] from `options`.
    ///
    /// Parses and merges all rule files.  Returns
    /// [`ObfuscationError::RuleFileNotFound`] if any rule file does not exist.
    pub fn new(options: ObfuscationOptions) -> Result<Self, ObfuscationError> {
        // Validate all rule files exist before doing any work.
        for rule in &options.rule_files {
            if !rule.exists() {
                return Err(ObfuscationError::RuleFileNotFound(rule.clone()));
            }
        }

        // Parse and merge.
        let merged = options.rule_files.iter().try_fold(
            ObfuscationRules::default(),
            |acc, path| -> Result<ObfuscationRules, ObfuscationError> {
                let r = ObfuscationRules::from_file(path)?;
                Ok(acc.merge(r))
            },
        )?;

        let hash = compute_rule_hash(&options.rule_files);

        Ok(Obfuscator {
            rules: merged,
            rule_hash_value: hash,
        })
    }

    /// Obfuscate JavaScript source text in memory.
    ///
    /// When `rules.enable_obfuscation` is `false` the source is returned
    /// unchanged — it is safe to call this unconditionally.
    pub fn obfuscate_source(
        &self,
        source: &str,
        filename: &str,
    ) -> Result<ObfuscationOutput, ObfuscationError> {
        self.rules.apply(source, filename)
    }

    /// Obfuscate a single `.js` file.
    ///
    /// Reads `input`, writes the result to `output`, and returns the output
    /// path on success.
    pub fn obfuscate_file(&self, input: &Path, output: &Path) -> Result<PathBuf, ObfuscationError> {
        let source = std::fs::read_to_string(input)?;
        let filename = input.to_string_lossy();
        let result = self.obfuscate_source(&source, &filename)?;
        std::fs::write(output, &result.code)?;
        Ok(output.to_path_buf())
    }

    /// Obfuscate all `.js` files found up to 2 levels deep in `dir`, in-place.
    ///
    /// Each file is written to a temporary path first and then atomically
    /// renamed over the original to avoid partial writes.
    pub fn obfuscate_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, ObfuscationError> {
        let mut outputs = Vec::new();
        for entry in walkdir::WalkDir::new(dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("js"))
        {
            let path = entry.path();
            let tmp = path.with_extension("js.obf_tmp");
            self.obfuscate_file(path, &tmp)?;
            std::fs::rename(&tmp, path)?;
            outputs.push(path.to_path_buf());
        }
        Ok(outputs)
    }

    /// FNV-1a hash of all combined rule file contents.
    ///
    /// Changes when any rule file is modified.  Use this value to decide
    /// whether to invalidate the `arkdown-abc` cache for release builds.
    pub fn rule_hash(&self) -> u64 {
        self.rule_hash_value
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Compute a FNV-1a hash of the contents of every rule file in `rule_files`.
///
/// Used for cache invalidation: if the hash changes between two builds, the
/// ABC compilation cache for all affected modules must be discarded.
pub fn compute_rule_hash(rule_files: &[PathBuf]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for rule in rule_files {
        if let Ok(content) = std::fs::read(rule) {
            for b in &content {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
    }
    h
}

/// Return `true` when obfuscation should run for this build.
///
/// Obfuscation is only performed in `Release` mode and when
/// `config.enable_obfuscation` is set to `true`.
pub fn should_obfuscate(build_mode: &BuildMode, config: Option<&ObfuscationConfig>) -> bool {
    config
        .map(|c| c.is_enabled_for_mode(build_mode))
        .unwrap_or(false)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // -- should_obfuscate -------------------------------------------------------

    #[test]
    fn test_should_not_obfuscate_debug() {
        assert!(!should_obfuscate(&BuildMode::Debug, None));
    }

    #[test]
    fn test_should_not_obfuscate_release_disabled() {
        let cfg = ObfuscationConfig {
            enable_obfuscation: false,
            rule_files: vec![],
        };
        assert!(!should_obfuscate(&BuildMode::Release, Some(&cfg)));
    }

    #[test]
    fn test_should_obfuscate_release_enabled() {
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![],
        };
        assert!(should_obfuscate(&BuildMode::Release, Some(&cfg)));
    }

    // -- compute_rule_hash -----------------------------------------------------

    #[test]
    fn test_rule_hash_deterministic() {
        let tmp = tempdir().expect("tempdir");
        let rule = tmp.path().join("rules.txt");
        std::fs::write(&rule, b"-enable-obfuscation\n").expect("write");
        let h1 = compute_rule_hash(&[rule.clone()]);
        let h2 = compute_rule_hash(&[rule]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_rule_hash_changes_with_content() {
        let tmp = tempdir().expect("tempdir");
        let rule = tmp.path().join("rules.txt");
        std::fs::write(&rule, b"-enable-obfuscation\n").expect("write");
        let h1 = compute_rule_hash(&[rule.clone()]);
        std::fs::write(&rule, b"-enable-obfuscation\n-compact\n").expect("write");
        let h2 = compute_rule_hash(&[rule]);
        assert_ne!(h1, h2);
    }

    // -- ObfuscationRules::parse -----------------------------------------------

    #[test]
    fn test_rule_parser_enable_flags() {
        let rules = ObfuscationRules::parse(
            "-enable-obfuscation\n-enable-property-obfuscation\n-compact\n-remove-log\n",
        );
        assert!(rules.enable_obfuscation);
        assert!(rules.enable_property_obfuscation);
        assert!(rules.compact);
        assert!(rules.remove_log);
        assert!(!rules.enable_toplevel_obfuscation);
    }

    #[test]
    fn test_keep_global_name_parsed() {
        let rules = ObfuscationRules::parse("-keep-global-name\nMyClass\nhelperFn\n");
        assert!(rules.keep_global_names.contains("MyClass"));
        assert!(rules.keep_global_names.contains("helperFn"));
    }

    #[test]
    fn test_keep_property_name_parsed() {
        let rules = ObfuscationRules::parse("-keep-property-name\npublicMethod\nonCreate\n");
        assert!(rules.keep_property_names.contains("publicMethod"));
        assert!(rules.keep_property_names.contains("onCreate"));
    }

    #[test]
    fn test_comment_and_blank_lines_ignored() {
        let rules = ObfuscationRules::parse("# this is a comment\n\n-enable-obfuscation\n\n");
        assert!(rules.enable_obfuscation);
    }

    #[test]
    fn test_disable_directive_overrides_enable() {
        let rules = ObfuscationRules::parse("-enable-obfuscation\n-disable-obfuscation\n");
        assert!(!rules.enable_obfuscation);
    }

    // -- Obfuscator integration ------------------------------------------------

    fn make_obfuscator(rules_str: &str, tmp: &tempfile::TempDir) -> Obfuscator {
        let rule_file = tmp.path().join("r.txt");
        std::fs::write(&rule_file, rules_str).expect("write rule file");
        Obfuscator::new(ObfuscationOptions {
            rule_files: vec![rule_file],
            cache_dir: tmp.path().to_path_buf(),
            export_rule_path: None,
        })
        .expect("Obfuscator::new")
    }

    #[test]
    fn test_no_rule_files_builds_empty_rules() {
        let tmp = tempdir().expect("tempdir");
        let opts = ObfuscationOptions {
            rule_files: vec![],
            cache_dir: tmp.path().to_path_buf(),
            export_rule_path: None,
        };
        let ob = Obfuscator::new(opts).expect("new");
        // With no rules, obfuscation is disabled — source passes through unchanged.
        let src = "var longVariableName = 42;";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        assert_eq!(out.code, src);
    }

    #[test]
    fn test_obfuscate_source_renames_local_when_enabled() {
        let tmp = tempdir().expect("tempdir");
        let ob = make_obfuscator("-enable-obfuscation\n", &tmp);
        let src = "var longVariableName = 1;";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        assert!(
            !out.code.contains("longVariableName"),
            "expected 'longVariableName' to be renamed, got: {}",
            out.code
        );
    }

    #[test]
    fn test_obfuscate_source_keeps_global_name() {
        let tmp = tempdir().expect("tempdir");
        let ob = make_obfuscator("-enable-obfuscation\n-keep-global-name\nMyClass\n", &tmp);
        let src = "class MyClass { }";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        assert!(
            out.code.contains("MyClass"),
            "expected 'MyClass' to be preserved, got: {}",
            out.code
        );
    }

    #[test]
    fn test_remove_log_strips_console_log() {
        let tmp = tempdir().expect("tempdir");
        let ob = make_obfuscator("-enable-obfuscation\n-remove-log\n", &tmp);
        let src = "var x = 1;\nconsole.log(x);\nvar y = 2;\n";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        assert!(
            !out.code.contains("console.log"),
            "expected console.log to be removed, got: {}",
            out.code
        );
        // At least one non-log variable should still be present (renamed to a/b).
        assert!(
            out.code.contains('a') || out.code.contains('b'),
            "non-log code should still be present: {}",
            out.code
        );
    }

    #[test]
    fn test_obfuscate_file_reads_and_writes() {
        let tmp = tempdir().expect("tempdir");
        let input = tmp.path().join("input.js");
        let output = tmp.path().join("output.js");
        std::fs::write(&input, "var longName = 1;").expect("write input");
        let ob = make_obfuscator("-enable-obfuscation\n", &tmp);
        let result_path = ob.obfuscate_file(&input, &output).expect("obfuscate_file");
        assert_eq!(result_path, output);
        let content = std::fs::read_to_string(&output).expect("read output");
        assert!(
            !content.contains("longName"),
            "longName should be obfuscated: {content}"
        );
    }

    #[test]
    fn test_rule_file_not_found_error() {
        let opts = ObfuscationOptions {
            rule_files: vec![PathBuf::from("/nonexistent/rules.txt")],
            cache_dir: PathBuf::from("/tmp"),
            export_rule_path: None,
        };
        let result = Obfuscator::new(opts);
        assert!(matches!(result, Err(ObfuscationError::RuleFileNotFound(_))));
    }

    #[test]
    fn test_function_name_renamed() {
        let tmp = tempdir().expect("tempdir");
        let ob = make_obfuscator("-enable-obfuscation\n", &tmp);
        let src = "function doSomethingComplex() { return 1; }";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        assert!(
            !out.code.contains("doSomethingComplex"),
            "function name should be renamed: {}",
            out.code
        );
    }

    #[test]
    fn test_property_access_not_renamed() {
        let tmp = tempdir().expect("tempdir");
        let ob = make_obfuscator("-enable-obfuscation\n", &tmp);
        // The binding `obj` gets renamed, but `.prop` must NOT be touched.
        let src = "var obj = {}; obj.prop = 1;";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        // `.prop` should still appear unchanged (it is a property access)
        assert!(
            out.code.contains(".prop"),
            "property access .prop must not be renamed: {}",
            out.code
        );
    }
}
