//! Hvigor-aligned obfuscation rule handling for arkdown release builds.
//!
//! The SDK implementation delegates source and bytecode obfuscation to
//! `arkguard`.  This crate parses and merges arkguard rule files, exports HAR
//! consumer `obfuscation.txt` files, and rejects source/bytecode options whose
//! full arkguard semantics have not been ported to Rust.  It intentionally does
//! not generate a local approximation for those options.

mod consumer;
pub mod plan;
mod rules;
mod transform;

pub use consumer::ConsumerObfuscationFileWriter;
pub use plan::{
    ConsumerObfuscationPlan, ObfuscationOutputPaths, ObfuscationPlan, ObfuscationRuleInput,
};
pub use rules::ObfuscationRules;
pub use transform::ObfuscationOutput;

use std::path::{Path, PathBuf};

use arkdown_path::ArtifactPathSerializer;

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
    #[error("Unsupported arkguard obfuscation options require full hvigor parity: {features}")]
    UnsupportedArkguardObfuscationOptions { features: String },
    #[error(transparent)]
    Path(#[from] arkdown_path::serializer::SerializerError),
}

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for a single obfuscation pass.
#[derive(Debug, Clone)]
pub struct ObfuscationOptions {
    /// `buildOption.arkOptions.obfuscation.ruleOptions.enable`.
    ///
    /// hvigor enables the pass from build-profile data; the rule file only
    /// carries detailed options and keep lists.
    pub enable_obfuscation: bool,
    /// Rule files whose directives control obfuscation behaviour.
    pub rule_files: Vec<PathBuf>,
    /// Consumer obfuscation configs exported by HAR/HSP dependencies.
    pub dependency_rule_files: Vec<PathBuf>,
    /// Cache directory for obfuscation outputs (used by the caller for
    /// incremental build cache invalidation; not written to by this crate).
    pub cache_dir: PathBuf,
    /// Export rule path (consumer rules for HAR publishing).
    pub export_rule_path: Option<PathBuf>,
}

// ── Obfuscator ────────────────────────────────────────────────────────────────

/// Hvigor-aligned obfuscation rule executor.
///
/// Create once per build (rules are parsed at construction time) and reuse
/// across all files in the same build. Active arkguard source/bytecode options
/// are rejected during construction until their full semantics are ported.
pub struct Obfuscator {
    rules: ObfuscationRules,
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
        for rule in &options.dependency_rule_files {
            if !rule.exists() {
                return Err(ObfuscationError::RuleFileNotFound(rule.clone()));
            }
        }

        // Parse and merge.
        let mut merged = options.rule_files.iter().try_fold(
            ObfuscationRules::default(),
            |acc, path| -> Result<ObfuscationRules, ObfuscationError> {
                let r = ObfuscationRules::from_file(path)?;
                Ok(acc.merge(r))
            },
        )?;
        let dependency_rules = options.dependency_rule_files.iter().try_fold(
            ObfuscationRules::default(),
            |acc, path| -> Result<ObfuscationRules, ObfuscationError> {
                let r = ObfuscationRules::from_file(path)?;
                Ok(acc.merge(r))
            },
        )?;
        let dependency_disables_obfuscation = dependency_rules.disable_obfuscation;
        merged = if merged.enable_lib_obfuscation_options {
            merged.merge(dependency_rules)
        } else {
            merged.merge_keep_options(dependency_rules)
        };
        merged.disable_obfuscation = merged.disable_obfuscation || dependency_disables_obfuscation;
        merged.enable_obfuscation = options.enable_obfuscation && !merged.disable_obfuscation;
        if merged.enable_obfuscation {
            let mut unsupported = vec!["arkguard-source-name-obfuscation"];
            unsupported.extend(merged.unsupported_arkguard_obfuscation_options());
            return Err(ObfuscationError::UnsupportedArkguardObfuscationOptions {
                features: unsupported.join(", "),
            });
        }

        Ok(Obfuscator { rules: merged })
    }

    /// Process JavaScript source text in memory.
    ///
    /// This returns source unchanged after construction has rejected unsupported
    /// arkguard source/bytecode options.
    pub fn obfuscate_source(
        &self,
        source: &str,
        filename: &str,
    ) -> Result<ObfuscationOutput, ObfuscationError> {
        self.rules.apply(source, filename)
    }

    /// Process a single `.js` file.
    ///
    /// Reads `input`, writes the result to `output`, and returns the output
    /// path on success.
    pub fn obfuscate_file(&self, input: &Path, output: &Path) -> Result<PathBuf, ObfuscationError> {
        let source = std::fs::read_to_string(input)?;
        let filename = ObfuscationPathText::new(input).string()?;
        let result = self.obfuscate_source(&source, filename.as_str())?;
        std::fs::write(output, &result.code)?;
        Ok(output.to_path_buf())
    }

    /// Process all `.js` files found up to 2 levels deep in `dir`, in-place.
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
}

struct ObfuscationPathText<'a> {
    path: &'a Path,
}

impl<'a> ObfuscationPathText<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn string(self) -> Result<String, ObfuscationError> {
        Ok(ArtifactPathSerializer::path_to_slash_string(self.path)?)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    // -- ObfuscationRules::parse -----------------------------------------------

    #[test]
    fn test_rule_parser_enable_flags() {
        let rules =
            ObfuscationRules::parse("-enable-property-obfuscation\n-compact\n-remove-log\n");
        assert!(!rules.enable_obfuscation);
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
        let rules = ObfuscationRules::parse("# this is a comment\n\n-compact\n\n");
        assert!(rules.compact);
    }

    #[test]
    fn test_disable_directive_overrides_enable() {
        let rules = ObfuscationRules::parse("-enable-property-obfuscation\n-disable-obfuscation\n");
        assert!(!rules.enable_obfuscation);
        assert!(rules.enable_property_obfuscation);
        assert!(rules.disable_obfuscation);
    }

    #[test]
    fn consumer_config_serializes_arkguard_exported_subset() {
        let rules = ObfuscationRules::parse(
            "-enable-property-obfuscation\n\
             -enable-string-property-obfuscation\n\
             -enable-toplevel-obfuscation\n\
             -enable-filename-obfuscation\n\
             -enable-export-obfuscation\n\
             -compact\n\
             -remove-log\n\
             -keep-global-name\n\
             Zebra\n\
             Alpha\n\
             -keep-property-name\n\
             beta\n\
             alpha\n",
        );

        assert_eq!(
            rules.serialize_consumer_config(),
            "-enable-property-obfuscation\n\
             -enable-string-property-obfuscation\n\
             -enable-toplevel-obfuscation\n\
             -compact\n\
             -remove-log\n\
             -keep-global-name\n\
             Alpha\n\
             Zebra\n\
             -keep-property-name\n\
             alpha\n\
             beta\n"
        );
    }

    #[test]
    fn consumer_writer_writes_obfuscation_txt_content() {
        let tmp = tempdir().unwrap();
        let consumer = tmp.path().join("consumer-rules.txt");
        let dependency = tmp.path().join("dep-obfuscation.txt");
        std::fs::write(
            &consumer,
            "-enable-property-obfuscation\n-keep-property-name\nfoo\n",
        )
        .expect("write consumer rules");
        std::fs::write(
            &dependency,
            "-enable-string-property-obfuscation\n-keep-property-name\nbar\n",
        )
        .expect("write dependency rules");
        let output = tmp.path().join("obfuscation.txt");

        ConsumerObfuscationFileWriter::new(vec![consumer], output.clone())
            .with_dependency_rule_files(vec![dependency])
            .write()
            .expect("write obfuscation.txt");

        let actual = std::fs::read_to_string(output).expect("read obfuscation.txt");
        assert_eq!(
            actual,
            "-enable-property-obfuscation\n-enable-string-property-obfuscation\n-keep-property-name\nbar\nfoo\n"
        );
    }

    #[test]
    fn dependency_rules_merge_keep_options_without_lib_obfuscation_flag() {
        let self_rules = ObfuscationRules::parse("-enable-property-obfuscation\n");
        let dependency_rules =
            ObfuscationRules::parse("-enable-toplevel-obfuscation\n-keep-global-name\nDepApi\n");

        let merged = self_rules.merge_keep_options(dependency_rules);

        assert!(merged.enable_property_obfuscation);
        assert!(!merged.enable_toplevel_obfuscation);
        assert!(merged.keep_global_names.contains("DepApi"));
    }

    // -- Obfuscator integration ------------------------------------------------

    fn make_obfuscator(
        rules_str: &str,
        tmp: &tempfile::TempDir,
    ) -> Result<Obfuscator, ObfuscationError> {
        let rule_file = tmp.path().join("r.txt");
        std::fs::write(&rule_file, rules_str).expect("write rule file");
        Obfuscator::new(ObfuscationOptions {
            enable_obfuscation: true,
            rule_files: vec![rule_file],
            dependency_rule_files: vec![],
            cache_dir: tmp.path().to_path_buf(),
            export_rule_path: None,
        })
    }

    #[test]
    fn test_no_rule_files_builds_empty_rules() {
        let tmp = tempdir().expect("tempdir");
        let opts = ObfuscationOptions {
            enable_obfuscation: false,
            rule_files: vec![],
            dependency_rule_files: vec![],
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
    fn build_profile_enable_requires_arkguard_source_obfuscation() {
        let tmp = tempdir().expect("tempdir");
        let err = match make_obfuscator("-keep-global-name\nKeptName\n", &tmp) {
            Ok(_) => panic!("expected unsupported arkguard source obfuscation error"),
            Err(err) => err,
        };
        assert!(
            matches!(
                err,
                ObfuscationError::UnsupportedArkguardObfuscationOptions { ref features }
                    if features.contains("arkguard-source-name-obfuscation")
            ),
            "expected source name obfuscation error, got {err:?}"
        );
    }

    #[test]
    fn unsupported_source_obfuscation_option_is_rejected() {
        let tmp = tempdir().expect("tempdir");
        let err = match make_obfuscator("-enable-property-obfuscation\n", &tmp) {
            Ok(_) => panic!("expected unsupported arkguard option error"),
            Err(err) => err,
        };
        assert!(
            matches!(
                err,
                ObfuscationError::UnsupportedArkguardObfuscationOptions { ref features }
                    if features.contains("-enable-property-obfuscation")
            ),
            "expected unsupported arkguard option error, got {err:?}"
        );
    }

    #[test]
    fn test_obfuscate_file_reads_and_writes() {
        let tmp = tempdir().expect("tempdir");
        let input = tmp.path().join("input.js");
        let output = tmp.path().join("output.js");
        std::fs::write(&input, "var longName = 1;").expect("write input");
        let rule_file = tmp.path().join("rules-disabled.txt");
        std::fs::write(&rule_file, "-keep-global-name\nlongName\n").expect("write rule");
        let ob = Obfuscator::new(ObfuscationOptions {
            enable_obfuscation: false,
            rule_files: vec![rule_file],
            dependency_rule_files: vec![],
            cache_dir: tmp.path().to_path_buf(),
            export_rule_path: None,
        })
        .expect("new");
        let result_path = ob.obfuscate_file(&input, &output).expect("obfuscate_file");
        assert_eq!(result_path, output);
        let content = std::fs::read_to_string(&output).expect("read output");
        assert_eq!(content, "var longName = 1;");
    }

    #[test]
    fn test_rule_file_not_found_error() {
        let opts = ObfuscationOptions {
            enable_obfuscation: true,
            rule_files: vec![PathBuf::from("/nonexistent/rules.txt")],
            dependency_rule_files: vec![],
            cache_dir: PathBuf::from("/tmp"),
            export_rule_path: None,
        };
        let result = Obfuscator::new(opts);
        assert!(matches!(result, Err(ObfuscationError::RuleFileNotFound(_))));
    }

    #[test]
    fn dependency_disable_obfuscation_suppresses_unsupported_option_rejection() {
        let tmp = tempdir().expect("tempdir");
        let rule_file = tmp.path().join("rules.txt");
        let dep_rule_file = tmp.path().join("dep-rules.txt");
        std::fs::write(&rule_file, "-enable-property-obfuscation\n").expect("write rule");
        std::fs::write(&dep_rule_file, "-disable-obfuscation\n").expect("write dep rule");

        let ob = Obfuscator::new(ObfuscationOptions {
            enable_obfuscation: true,
            rule_files: vec![rule_file],
            dependency_rule_files: vec![dep_rule_file],
            cache_dir: tmp.path().to_path_buf(),
            export_rule_path: None,
        })
        .expect("dependency disable should match arkguard ConfigResolver");
        let src = "var longName = 1;";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        assert_eq!(out.code, src);
    }
}
