//! Hvigor-aligned obfuscation rule handling for arkdown release builds.
//!
//! The SDK implementation delegates source and bytecode obfuscation to
//! `arkguard`. This crate parses and merges arkguard rule files, exports HAR
//! consumer `obfuscation.txt` files, and invokes the SDK ArkGuard package for
//! module-scoped source obfuscation and the SDK bytecode obfuscator after
//! `es2abc` has produced `modules.abc`.

mod arkguard;
mod bytecode;
mod consumer;
pub mod plan;
mod rules;
mod transform;

pub use arkguard::{
    ArkguardAdapterOptions, ObfuscationModuleFile, ObfuscationModuleInput, ObfuscationModuleOutput,
};
pub use bytecode::{
    BytecodeObfuscationInput, BytecodeObfuscationModuleRecord, BytecodeObfuscationOutput,
};
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
    #[error("SDK ArkGuard package does not exist: {0}")]
    ArkguardPackageNotFound(PathBuf),
    #[error("SDK ArkGuard API declaration roots do not exist: {0:?}")]
    ArkguardSdkApisNotFound(Vec<PathBuf>),
    #[error("active ArkGuard source obfuscation requires a complete module input")]
    ArkguardModuleContextRequired,
    #[error("bytecode obfuscation must run after es2abc with a complete modules.abc input")]
    BytecodeObfuscationRequiresPostCompile,
    #[error("bytecode obfuscation is not enabled by the effective ArkGuard rules")]
    BytecodeObfuscationNotEnabled,
    #[error(
        "bytecode obfuscation requires API 13 or API 12 beta3, got compatibleSdkVersion={version} stage={stage:?}"
    )]
    BytecodeObfuscationUnsupportedCompatibleSdk { version: u32, stage: Option<String> },
    #[error("SDK ets-loader directory does not exist: {0}")]
    EtsLoaderNotFound(PathBuf),
    #[error("bytecode obfuscation input does not exist: {0}")]
    BytecodeObfuscationInputNotFound(PathBuf),
    #[error("bytecode obfuscation requires sourceMaps.map from the bundle stage")]
    BytecodeObfuscationSourceMapsRequired,
    #[error("bytecode obfuscation expected ABC at {expected}, got {actual}")]
    BytecodeObfuscationAbcPathMismatch { expected: PathBuf, actual: PathBuf },
    #[error("bytecode obfuscation output does not exist: {0}")]
    BytecodeObfuscationOutputNotFound(PathBuf),
    #[error("failed to start ArkGuard process '{executable}': {source}")]
    ArkguardProcessStart {
        executable: PathBuf,
        source: std::io::Error,
    },
    #[error("ArkGuard process failed with status {status:?}: {stderr}")]
    ArkguardProcessFailed { status: Option<i32>, stderr: String },
    #[error("ArkGuard request/response JSON error: {0}")]
    Json(#[from] serde_json::Error),
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
    /// Typed adapter configuration for the SDK ArkGuard implementation.
    /// Required when source or bytecode obfuscation is enabled.
    pub arkguard: Option<ArkguardAdapterOptions>,
}

// ── Obfuscator ────────────────────────────────────────────────────────────────

/// Hvigor-aligned obfuscation rule executor.
///
/// Create once per build (rules are parsed at construction time) and reuse
/// across all files in the same module. Active source options are delegated to
/// the SDK ArkGuard implementation. Bytecode obfuscation is exposed as a
/// separate post-`es2abc` operation so source and bytecode modes cannot run at
/// the same time, matching SDK `Initializer.initObfuscationConfig`.
pub struct Obfuscator {
    rules: ObfuscationRules,
    options: ObfuscationOptions,
}

/// Effective execution mode selected by the merged ArkGuard rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObfuscationMode {
    Disabled,
    Source,
    Bytecode,
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
        Ok(Obfuscator {
            rules: merged,
            options,
        })
    }

    /// Return the SDK execution mode selected by the effective rule set.
    pub fn mode(&self) -> ObfuscationMode {
        if !self.rules.enable_obfuscation {
            ObfuscationMode::Disabled
        } else if self.rules.enable_bytecode_obfuscation {
            ObfuscationMode::Bytecode
        } else {
            ObfuscationMode::Source
        }
    }

    /// Process JavaScript source text in memory.
    ///
    /// Disabled configurations return source unchanged. Active source
    /// obfuscation requires [`Self::obfuscate_module`] so ArkGuard receives the
    /// full module inventory, source maps, SDK APIs, and shared name cache.
    pub fn obfuscate_source(
        &self,
        source: &str,
        filename: &str,
    ) -> Result<ObfuscationOutput, ObfuscationError> {
        if self.rules.enable_obfuscation {
            return Err(ObfuscationError::ArkguardModuleContextRequired);
        }
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

    /// Obfuscate all CompileArkTS sources for one module in a single ArkGuard
    /// process so the SDK's ordered name generator and global caches are shared
    /// across files exactly as they are in hvigor.
    pub fn obfuscate_module(
        &self,
        input: ObfuscationModuleInput,
    ) -> Result<ObfuscationModuleOutput, ObfuscationError> {
        if self.mode() == ObfuscationMode::Disabled {
            return Ok(ObfuscationModuleOutput {
                name_cache_path: self.options.cache_dir.join("nameCache.json"),
                output_files: input
                    .files
                    .into_iter()
                    .map(|file| file.build_path)
                    .collect(),
            });
        }
        if self.mode() == ObfuscationMode::Bytecode {
            return Err(ObfuscationError::BytecodeObfuscationRequiresPostCompile);
        }
        let adapter = self
            .options
            .arkguard
            .as_ref()
            .ok_or_else(|| ObfuscationError::ArkguardPackageNotFound(PathBuf::new()))?;
        arkguard::ArkguardModuleRunner::new(&self.options, adapter).run(input)
    }

    /// Obfuscate a completed `modules.abc` with the latest SDK's ArkGuard and
    /// PandaGuard implementation.
    pub fn obfuscate_bytecode(
        &self,
        input: BytecodeObfuscationInput,
    ) -> Result<BytecodeObfuscationOutput, ObfuscationError> {
        if self.mode() != ObfuscationMode::Bytecode {
            return Err(ObfuscationError::BytecodeObfuscationNotEnabled);
        }
        let adapter = self
            .options
            .arkguard
            .as_ref()
            .ok_or_else(|| ObfuscationError::ArkguardPackageNotFound(PathBuf::new()))?;
        bytecode::ArkguardBytecodeRunner::new(&self.options, adapter).run(input)
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
            arkguard: None,
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
            arkguard: None,
        };
        let ob = Obfuscator::new(opts).expect("new");
        // With no rules, obfuscation is disabled — source passes through unchanged.
        let src = "var longVariableName = 42;";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        assert_eq!(out.code, src);
    }

    #[test]
    fn active_source_obfuscation_requires_complete_module_context() {
        let tmp = tempdir().expect("tempdir");
        let obfuscator = make_obfuscator("-keep-global-name\nKeptName\n", &tmp)
            .expect("active source rules should initialize");
        let err = obfuscator
            .obfuscate_source("const SourceName = 1;", "Index.ts")
            .expect_err("single-file API lacks ArkGuard module context");
        assert!(
            matches!(err, ObfuscationError::ArkguardModuleContextRequired),
            "expected module-context error, got {err:?}"
        );
    }

    #[test]
    fn property_obfuscation_is_delegated_to_sdk_arkguard() {
        let tmp = tempdir().expect("tempdir");
        let obfuscator = make_obfuscator("-enable-property-obfuscation\n", &tmp)
            .expect("SDK ArkGuard supports property obfuscation");
        let err = obfuscator
            .obfuscate_source("const SourceName = 1;", "Index.ts")
            .expect_err("single-file API must not approximate module semantics");
        assert!(
            matches!(err, ObfuscationError::ArkguardModuleContextRequired),
            "expected module-context error, got {err:?}"
        );
    }

    #[test]
    fn bytecode_rule_selects_post_es2abc_mode_without_source_rewrite() {
        let tmp = tempdir().expect("tempdir");
        let obfuscator = make_obfuscator(
            "-enable-property-obfuscation\n-enable-bytecode-obfuscation\n",
            &tmp,
        )
        .expect("SDK bytecode mode should be accepted");
        assert_eq!(obfuscator.mode(), ObfuscationMode::Bytecode);
        let err = obfuscator
            .obfuscate_module(ObfuscationModuleInput {
                files: Vec::new(),
                source_maps_path: None,
            })
            .expect_err("bytecode mode must wait for modules.abc");
        assert!(matches!(
            err,
            ObfuscationError::BytecodeObfuscationRequiresPostCompile
        ));
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
            arkguard: None,
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
            arkguard: None,
        };
        let result = Obfuscator::new(opts);
        assert!(matches!(result, Err(ObfuscationError::RuleFileNotFound(_))));
    }

    #[test]
    fn dependency_disable_obfuscation_suppresses_active_sdk_mode() {
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
            arkguard: None,
        })
        .expect("dependency disable should match arkguard ConfigResolver");
        let src = "var longName = 1;";
        let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
        assert_eq!(out.code, src);
    }
}
