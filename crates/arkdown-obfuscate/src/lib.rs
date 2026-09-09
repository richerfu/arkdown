//! Hvigor-aligned obfuscation rule handling for arkdown release builds.
//!
//! Source obfuscation is implemented natively on OXC AST and semantic data.
//! Bytecode obfuscation invokes the SDK's native PandaGuard executable after
//! `es2abc` has produced `modules.abc`. The crate also owns ArkGuard rule
//! merging, cache/source-map publication, and HAR consumer rules.

mod arkguard;
#[cfg(test)]
mod arkguard_tests;
mod bytecode;
#[cfg(test)]
mod bytecode_tests;
mod consumer;
pub mod plan;
mod rules;

pub use arkguard::{
    ArkguardAdapterOptions, ObfuscationModuleFile, ObfuscationModuleFileOutput,
    ObfuscationModuleInput, ObfuscationModuleOutput, ObfuscationSource, ObfuscationSourceKind,
};
pub use bytecode::{
    BytecodeObfuscationInput, BytecodeObfuscationModuleRecord, BytecodeObfuscationOutput,
};
pub use consumer::ConsumerObfuscationFileWriter;
pub use plan::{
    ConsumerObfuscationPlan, ObfuscationOutputPaths, ObfuscationPlan, ObfuscationRuleInput,
};
pub use rules::ObfuscationRules;

use std::path::PathBuf;

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
    #[error("ArkGuard name cache does not exist: {0}")]
    NameCacheNotFound(PathBuf),
    #[error("ArkGuard -keep-dts path does not exist: {0}")]
    KeepDtsNotFound(PathBuf),
    #[error("ArkGuard name cache contains an unsafe artifact path: {0}")]
    UnsafeNameCachePath(String),
    #[error("ArkGuard produced no output mapping for CompileArkTS source: {0}")]
    ArkguardOutputMappingMissing(PathBuf),
    #[error("SDK ArkGuard package does not exist: {0}")]
    ArkguardPackageNotFound(PathBuf),
    #[error("ArkGuard build metadata is required for active obfuscation")]
    ArkguardConfigurationRequired,
    #[error("bytecode obfuscation must run after es2abc with a complete modules.abc input")]
    BytecodeObfuscationRequiresPostCompile,
    #[error("bytecode obfuscation is not enabled by the effective ArkGuard rules")]
    BytecodeObfuscationNotEnabled,
    #[error(
        "bytecode obfuscation requires API 13 or API 12 beta3, got compatibleSdkVersion={version} stage={stage:?}"
    )]
    BytecodeObfuscationUnsupportedCompatibleSdk { version: u32, stage: Option<String> },
    #[error("bytecode obfuscation input does not exist: {0}")]
    BytecodeObfuscationInputNotFound(PathBuf),
    #[error("bytecode obfuscation requires sourceMaps.map from the bundle stage")]
    BytecodeObfuscationSourceMapsRequired,
    #[error("bytecode obfuscation expected ABC at {expected}, got {actual}")]
    BytecodeObfuscationAbcPathMismatch { expected: PathBuf, actual: PathBuf },
    #[error("bytecode obfuscation output does not exist: {0}")]
    BytecodeObfuscationOutputNotFound(PathBuf),
    #[error("invalid source map for bytecode obfuscation input '{source_file}': {message}")]
    BytecodeObfuscationSourceMapInvalid {
        source_file: String,
        message: String,
    },
    #[error("failed to start PandaGuard process '{executable}': {source}")]
    PandaGuardProcessStart {
        executable: PathBuf,
        source: std::io::Error,
    },
    #[error("PandaGuard process failed with status {status:?}: {stderr}")]
    PandaGuardProcessFailed { status: Option<i32>, stderr: String },
    #[error("obfuscation JSON error: {0}")]
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
    /// Typed native ArkGuard/PandaGuard build metadata.
    /// Required when source or bytecode obfuscation is enabled.
    pub arkguard: Option<ArkguardAdapterOptions>,
}

// ── Obfuscator ────────────────────────────────────────────────────────────────

/// Hvigor-aligned obfuscation rule executor.
///
/// Create once per build (rules are parsed at construction time) and reuse
/// across all files in the same module. Source transforms use OXC natively.
/// Bytecode obfuscation is exposed as a
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
        merged.resolve_keep_dts_names()?;
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

    /// Attach frontend-specific SDK metadata after rule parsing selected the
    /// execution mode. This avoids building the dynamic ets-loader inventory
    /// for ArkTS 1.2, whose bytecode configuration is owned by the static SDK
    /// driver instead.
    pub fn with_arkguard(mut self, arkguard: ArkguardAdapterOptions) -> Self {
        self.options.arkguard = Some(arkguard);
        self
    }

    /// Obfuscate all CompileArkTS sources for one module while sharing the
    /// ordered name generators and global caches used by ArkGuard.
    pub fn obfuscate_module(
        &self,
        input: ObfuscationModuleInput,
    ) -> Result<ObfuscationModuleOutput, ObfuscationError> {
        if self.mode() == ObfuscationMode::Disabled {
            return Ok(ObfuscationModuleOutput {
                name_cache_path: self.options.cache_dir.join("nameCache.json"),
                files: input
                    .files
                    .into_iter()
                    .map(|file| ObfuscationModuleFileOutput {
                        input_path: file.build_path.clone(),
                        output_path: file.build_path,
                        source_map_key: file.source_map_key.clone(),
                        obfuscated_source_map_key: file.source_map_key,
                        record_name: file.record_name.clone(),
                        obfuscated_record_name: file.record_name,
                    })
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
            .ok_or(ObfuscationError::ArkguardConfigurationRequired)?;
        arkguard::ArkguardModuleRunner::new(&self.options, &self.rules, adapter)?.run(input)
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
            .ok_or(ObfuscationError::ArkguardConfigurationRequired)?;
        if input
            .module
            .files
            .iter()
            .any(ObfuscationModuleFile::is_declaration)
        {
            let mut declarations = input.module.clone();
            declarations.source_maps_path = None;
            declarations.source_map_mirror_paths.clear();
            arkguard::ArkguardModuleRunner::new(&self.options, &self.rules, adapter)?
                .run_declarations(declarations)?;
        }
        bytecode::ArkguardBytecodeRunner::new(&self.options, &self.rules, adapter).run(input)
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
    fn rule_parser_preserves_property_syntax_for_removed_calls() {
        let rules = ObfuscationRules::parse(
            "-enable-property-obfuscation'-remove-nosideeffects-calls class['property?']*'\
             -enable-toplevel-obfuscation'-remove-nosideeffects-calls class.method",
        );

        assert!(rules.enable_property_obfuscation);
        assert!(rules.enable_toplevel_obfuscation);
        assert!(rules
            .remove_no_side_effect_calls
            .contains("class['property?']*"));
        assert!(rules.remove_no_side_effect_calls.contains("class.method"));
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
        assert_eq!(ob.mode(), ObfuscationMode::Disabled);
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
                inventory_sources: Vec::new(),
                module_dependencies: std::collections::BTreeMap::new(),
                preserved_source_maps: std::collections::BTreeMap::new(),
                source_maps_path: None,
                source_map_mirror_paths: Vec::new(),
            })
            .expect_err("bytecode mode must wait for modules.abc");
        assert!(matches!(
            err,
            ObfuscationError::BytecodeObfuscationRequiresPostCompile
        ));
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
        assert_eq!(ob.mode(), ObfuscationMode::Disabled);
    }
}
