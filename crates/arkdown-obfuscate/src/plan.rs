//! Obfuscation plan — domain type describing *what* to obfuscate and *where*
//! to write outputs.
//!
//! # Responsibilities
//!
//! `ObfuscationPlan` is the pure-data counterpart to hvigor's
//! `ArkCompile.getObfuscationOptions()` and `computeObfuscationFileHash()`.
//! It is constructed by the build service's `BuildPlanBuilder` (which has access to the
//! session, SDK paths, and resolved file inventories) and then handed to
//! [`crate::Obfuscator`] for execution.
//!
//! **Domain constraint**: this file must NOT depend on `arkdown-session`,
//! `arkdown-config`, or any other session/build-service crate.  All paths are
//! pre-resolved by the caller before the plan is constructed.
//!
//! # hvigor equivalents
//!
//! | hvigor                              | this file                                  |
//! |-------------------------------------|--------------------------------------------|
//! | `getObfuscationOptions()`           | [`ObfuscationPlan::from_build_options()`]  |
//! | `computeObfuscationFileHash()`      | [`ObfuscationPlan::rule_fingerprint()`]    |
//! | `obfuscationOptions.selfConfig`     | [`ObfuscationPlan::rule_inputs`]           |
//! | `obfuscationOptions.exportRulePath` | [`ObfuscationOutputPaths::export_rule`]    |
//! | `ProcessObfuscationFiles` (HAR)     | [`ConsumerObfuscationPlan`]                |

use std::path::PathBuf;

use arkdown_core::{BuildMode, ObfuscationConfig};

use crate::ObfuscationError;

// ── ObfuscationRuleInput ──────────────────────────────────────────────────────

/// A single resolved rule-file entry, optionally accompanied by rule files
/// from HAR/HSP dependencies that must be merged before obfuscation runs.
///
/// Corresponds to one element of `selfConfig.ruleOptions.rules` plus the
/// dependency rule files that hvigor collects from `dependencies.hars` and
/// `dependencies.hsps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObfuscationRuleInput {
    /// Absolute path to the primary rule file for this module.
    pub rule_file: PathBuf,
    /// Absolute paths to consumer-rule files exported by HAR/HSP dependencies.
    ///
    /// These are merged into the effective rule set before the obfuscation pass
    /// runs, matching hvigor's `ProcessObfuscationFiles` behaviour.
    pub dependency_rule_files: Vec<PathBuf>,
}

impl ObfuscationRuleInput {
    /// Create a rule input with no dependency rules.
    pub fn new(rule_file: PathBuf) -> Self {
        Self {
            rule_file,
            dependency_rule_files: Vec::new(),
        }
    }

    /// Create a rule input with pre-resolved dependency rule files.
    pub fn with_dependencies(rule_file: PathBuf, dependency_rule_files: Vec<PathBuf>) -> Self {
        Self {
            rule_file,
            dependency_rule_files,
        }
    }

    /// Collect all paths belonging to this input (primary + dependencies).
    ///
    /// Used by [`ObfuscationPlan::rule_fingerprint()`] to hash every file that
    /// influences the effective rule set.
    pub fn all_paths(&self) -> impl Iterator<Item = &PathBuf> {
        std::iter::once(&self.rule_file).chain(self.dependency_rule_files.iter())
    }

    /// Return `true` if the primary rule file exists on disk.
    pub fn exists(&self) -> bool {
        self.rule_file.exists()
    }
}

// ── ObfuscationOutputPaths ────────────────────────────────────────────────────

/// Resolved output paths produced by the obfuscation stage.
///
/// All paths are absolute and must be created by the caller before the
/// obfuscation pass writes to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObfuscationOutputPaths {
    /// Directory where obfuscated `.js` chunks are written.
    ///
    /// Corresponds to `obfuscationOptions.obfuscationCacheDir` in hvigor.
    pub obfuscation_cache_dir: PathBuf,
    /// Path where the consumer-facing obfuscation rule file is written.
    ///
    /// Written for HAR publishing so downstream consumers can merge the rules.
    /// Corresponds to `obfuscationOptions.exportRulePath`.
    ///
    /// `None` for non-HAR builds that do not publish consumer rules.
    pub export_rule: Option<PathBuf>,
}

impl ObfuscationOutputPaths {
    /// Create output paths with no consumer rule export.
    pub fn new(obfuscation_cache_dir: PathBuf) -> Self {
        Self {
            obfuscation_cache_dir,
            export_rule: None,
        }
    }

    /// Create output paths with a consumer rule export path.
    pub fn with_export_rule(obfuscation_cache_dir: PathBuf, export_rule: PathBuf) -> Self {
        Self {
            obfuscation_cache_dir,
            export_rule: Some(export_rule),
        }
    }
}

// ── ConsumerObfuscationPlan ───────────────────────────────────────────────────

/// Plan for generating consumer-facing obfuscation configuration for HAR
/// publishing.
///
/// When a HAR module is built with obfuscation enabled, the effective consumer
/// rules must be written to a file that is bundled into the HAR archive.
/// Downstream consumers merge these rules into their own obfuscation config.
///
/// Corresponds to hvigor's `ProcessObfuscationFiles` task and
/// `load-sdk-functions.js::generateObfuscationFile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerObfuscationPlan {
    /// Absolute paths to consumer rule files declared in
    /// `buildOption.arkOptions.obfuscation.consumerRules`.
    pub consumer_rule_files: Vec<PathBuf>,
    /// Absolute path where the merged consumer rule file is written.
    ///
    /// hvigor writes `<build>/default/intermediates/obfuscation/<target>/obfuscation.txt`.
    pub output_path: PathBuf,
}

impl ConsumerObfuscationPlan {
    /// Create a new consumer obfuscation plan.
    pub fn new(consumer_rule_files: Vec<PathBuf>, output_path: PathBuf) -> Self {
        Self {
            consumer_rule_files,
            output_path,
        }
    }

    /// Return `true` when there is at least one consumer rule file to export.
    pub fn has_consumer_rules(&self) -> bool {
        !self.consumer_rule_files.is_empty()
    }
}

// ── ObfuscationPlan ───────────────────────────────────────────────────────────

/// A fully resolved, validated plan describing the obfuscation pass for a
/// single build.
///
/// Constructed by the build service's `BuildPlanBuilder` from the parsed
/// `build-profile.json5` and session state.  Passed to [`crate::Obfuscator`]
/// for execution.
///
/// ## hvigor equivalents
///
/// ```text
/// hvigor ArkCompile.beforeAlwaysAction()
///   └─ getObfuscationOptions()       →  ObfuscationPlan::from_build_options()
///   └─ computeObfuscationFileHash()  →  ObfuscationPlan::rule_fingerprint()
/// ```
#[derive(Debug, Clone)]
pub struct ObfuscationPlan {
    /// Whether obfuscation is active for this build.
    ///
    /// `false` means the [`crate::Obfuscator`] should pass all source through
    /// unchanged.  This field is derived from `BuildMode` and the
    /// `ObfuscationConfig.enable_obfuscation` flag.
    pub is_enabled: bool,
    /// Resolved rule file inputs for this module.
    ///
    /// Empty when `is_enabled` is `false`.
    pub rule_inputs: Vec<ObfuscationRuleInput>,
    /// Resolved output paths for obfuscated artifacts.
    pub output_paths: ObfuscationOutputPaths,
    /// Optional consumer obfuscation plan for HAR publishing.
    ///
    /// `None` for non-HAR builds.
    pub consumer_plan: Option<ConsumerObfuscationPlan>,
}

impl ObfuscationPlan {
    /// Construct an [`ObfuscationPlan`] from an [`ObfuscationConfig`] and
    /// build mode.
    ///
    /// Rule file paths in `config.rule_files` must already be absolute and
    /// pre-resolved by the caller (typically `BuildPlanBuilder`).  No
    /// filesystem access is performed here.
    ///
    /// Corresponds to hvigor's `ArkCompile.getObfuscationOptions()`.
    pub fn from_build_options(
        build_mode: &BuildMode,
        config: Option<&ObfuscationConfig>,
        output_paths: ObfuscationOutputPaths,
        consumer_plan: Option<ConsumerObfuscationPlan>,
    ) -> Self {
        let is_enabled = config
            .map(|c| c.is_enabled_for_mode(build_mode))
            .unwrap_or(false);

        let rule_inputs = if is_enabled {
            config
                .map(|c| {
                    c.rule_files
                        .iter()
                        .map(|p| ObfuscationRuleInput::new(p.clone()))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Self {
            is_enabled,
            rule_inputs,
            output_paths,
            consumer_plan,
        }
    }

    /// Create a disabled (no-op) plan.
    ///
    /// Used in debug builds or when obfuscation is not configured.
    pub fn disabled(output_paths: ObfuscationOutputPaths) -> Self {
        Self {
            is_enabled: false,
            rule_inputs: Vec::new(),
            output_paths,
            consumer_plan: None,
        }
    }

    /// Returns `true` when obfuscation should actually run.
    ///
    /// Equivalent to checking `is_enabled` but expressed as a named predicate
    /// for call sites that read more naturally as `plan.should_obfuscate()`.
    #[inline]
    pub fn should_obfuscate(&self) -> bool {
        self.is_enabled
    }

    /// Validate the plan, returning an error if any required rule file is
    /// missing from disk.
    ///
    /// Call this after construction and before passing the plan to
    /// [`crate::Obfuscator`].  When `is_enabled` is `false` the method
    /// succeeds immediately without touching the filesystem.
    pub fn validate(&self) -> Result<(), ObfuscationError> {
        if !self.is_enabled {
            return Ok(());
        }
        for input in &self.rule_inputs {
            if !input.rule_file.exists() {
                return Err(ObfuscationError::RuleFileNotFound(input.rule_file.clone()));
            }
            for dep in &input.dependency_rule_files {
                if !dep.exists() {
                    return Err(ObfuscationError::RuleFileNotFound(dep.clone()));
                }
            }
        }
        if let Some(consumer) = &self.consumer_plan {
            for path in &consumer.consumer_rule_files {
                if !path.exists() {
                    return Err(ObfuscationError::RuleFileNotFound(path.clone()));
                }
            }
        }
        Ok(())
    }

    /// Compute a FNV-1a fingerprint over the contents of every rule file in
    /// this plan (primary rules + dependency rules).
    ///
    /// The fingerprint changes whenever any rule file is added, removed, or
    /// modified.  Use this value to invalidate the SDK es2abc compilation
    /// cache for release builds.
    ///
    /// Corresponds to hvigor's `ArkCompile.computeObfuscationFileHash()`.
    ///
    /// Returns `0` when `is_enabled` is `false` (no files to hash).
    pub fn rule_fingerprint(&self) -> u64 {
        if !self.is_enabled {
            return 0;
        }
        let all_paths: Vec<&PathBuf> = self
            .rule_inputs
            .iter()
            .flat_map(|input| input.all_paths())
            .chain(
                self.consumer_plan
                    .iter()
                    .flat_map(|consumer| consumer.consumer_rule_files.iter()),
            )
            .collect();
        fnv1a_hash_files(&all_paths)
    }

    /// Collect the flat list of all primary rule file paths in this plan.
    ///
    /// Convenience accessor for callers that need to pass rule files to
    /// [`crate::ObfuscationOptions`].
    pub fn all_rule_files(&self) -> Vec<PathBuf> {
        self.rule_inputs
            .iter()
            .flat_map(|input| input.all_paths().cloned())
            .collect()
    }
}

// ── FNV-1a helper ─────────────────────────────────────────────────────────────

/// Compute a FNV-1a hash of the contents of every path in `paths`.
///
/// Missing files are silently skipped (matching hvigor's `fse.existsSync`
/// guard in `computeObfuscationFileHash`).
fn fnv1a_hash_files(paths: &[&PathBuf]) -> u64 {
    let mut state = arkdown_core::fnv1a64(&[]);
    for path in paths {
        if let Ok(content) = std::fs::read(path) {
            state = arkdown_core::fnv1a64_fold(state, &content);
        }
    }
    state
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_rule_file(dir: &std::path::Path, name: &str, content: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).expect("write rule file");
        p
    }

    fn default_output_paths(dir: &std::path::Path) -> ObfuscationOutputPaths {
        ObfuscationOutputPaths::new(dir.join("obf-cache"))
    }

    // -- should_obfuscate / is_enabled -----------------------------------------

    #[test]
    fn plan_disabled_for_debug_build() {
        let tmp = tempdir().unwrap();
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![],
            consumer_rule_files: vec![],
        };
        let plan = ObfuscationPlan::from_build_options(
            &BuildMode::Debug,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        assert!(!plan.is_enabled);
        assert!(!plan.should_obfuscate());
    }

    #[test]
    fn plan_disabled_when_config_absent() {
        let tmp = tempdir().unwrap();
        let plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            None,
            default_output_paths(tmp.path()),
            None,
        );
        assert!(!plan.is_enabled);
    }

    #[test]
    fn plan_disabled_when_enable_false() {
        let tmp = tempdir().unwrap();
        let cfg = ObfuscationConfig {
            enable_obfuscation: false,
            rule_files: vec![],
            consumer_rule_files: vec![],
        };
        let plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        assert!(!plan.is_enabled);
    }

    #[test]
    fn plan_enabled_for_release_with_enable_true() {
        let tmp = tempdir().unwrap();
        let rule = make_rule_file(tmp.path(), "rules.txt", b"-keep-global-name\nPlanRule\n");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![rule],
            consumer_rule_files: vec![],
        };
        let plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        assert!(plan.is_enabled);
        assert!(plan.should_obfuscate());
        assert_eq!(plan.rule_inputs.len(), 1);
    }

    // -- disabled() constructor ------------------------------------------------

    #[test]
    fn disabled_plan_has_no_rule_inputs() {
        let tmp = tempdir().unwrap();
        let plan = ObfuscationPlan::disabled(default_output_paths(tmp.path()));
        assert!(!plan.is_enabled);
        assert!(plan.rule_inputs.is_empty());
        assert!(plan.consumer_plan.is_none());
    }

    // -- validate() ------------------------------------------------------------

    #[test]
    fn validate_succeeds_when_disabled() {
        let tmp = tempdir().unwrap();
        let plan = ObfuscationPlan::disabled(default_output_paths(tmp.path()));
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn validate_succeeds_when_rule_files_exist() {
        let tmp = tempdir().unwrap();
        let rule = make_rule_file(tmp.path(), "rules.txt", b"-keep-global-name\nPlanRule\n");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![rule],
            consumer_rule_files: vec![],
        };
        let plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn validate_errors_on_missing_rule_file() {
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("nonexistent.txt");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![missing.clone()],
            consumer_rule_files: vec![],
        };
        let plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        let err = plan.validate().unwrap_err();
        assert!(
            matches!(err, ObfuscationError::RuleFileNotFound(p) if p == missing),
            "expected RuleFileNotFound for missing rule"
        );
    }

    #[test]
    fn validate_errors_on_missing_dependency_rule_file() {
        let tmp = tempdir().unwrap();
        let rule = make_rule_file(tmp.path(), "rules.txt", b"-keep-global-name\nPlanRule\n");
        let missing_dep = tmp.path().join("dep-missing.txt");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![rule],
            consumer_rule_files: vec![],
        };
        let mut plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        // Inject a missing dependency rule file.
        plan.rule_inputs[0]
            .dependency_rule_files
            .push(missing_dep.clone());

        let err = plan.validate().unwrap_err();
        assert!(
            matches!(err, ObfuscationError::RuleFileNotFound(p) if p == missing_dep),
            "expected RuleFileNotFound for missing dep rule"
        );
    }

    #[test]
    fn validate_errors_on_missing_consumer_rule_file() {
        let tmp = tempdir().unwrap();
        let rule = make_rule_file(tmp.path(), "rules.txt", b"-keep-global-name\nPlanRule\n");
        let missing_consumer = tmp.path().join("consumer-missing.txt");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![rule],
            consumer_rule_files: vec![],
        };
        let consumer_plan = ConsumerObfuscationPlan::new(
            vec![missing_consumer.clone()],
            tmp.path().join("out-consumer.txt"),
        );
        let plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            Some(consumer_plan),
        );
        let err = plan.validate().unwrap_err();
        assert!(
            matches!(err, ObfuscationError::RuleFileNotFound(p) if p == missing_consumer),
            "expected RuleFileNotFound for missing consumer rule"
        );
    }

    // -- rule_fingerprint() ----------------------------------------------------

    #[test]
    fn fingerprint_is_zero_when_disabled() {
        let tmp = tempdir().unwrap();
        let plan = ObfuscationPlan::disabled(default_output_paths(tmp.path()));
        assert_eq!(plan.rule_fingerprint(), 0);
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let tmp = tempdir().unwrap();
        let rule = make_rule_file(tmp.path(), "rules.txt", b"-keep-global-name\nPlanRule\n");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![rule],
            consumer_rule_files: vec![],
        };
        let plan1 = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        let plan2 = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        assert_eq!(plan1.rule_fingerprint(), plan2.rule_fingerprint());
    }

    #[test]
    fn fingerprint_changes_when_rule_content_changes() {
        let tmp = tempdir().unwrap();
        let rule = make_rule_file(tmp.path(), "rules.txt", b"-keep-global-name\nPlanRule\n");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![rule.clone()],
            consumer_rule_files: vec![],
        };
        let plan1 = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        let fp1 = plan1.rule_fingerprint();

        // Modify the rule file.
        std::fs::write(&rule, b"-keep-global-name\nPlanRule\n-compact\n").unwrap();

        let plan2 = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        let fp2 = plan2.rule_fingerprint();

        assert_ne!(
            fp1, fp2,
            "fingerprint must change when rule content changes"
        );
    }

    #[test]
    fn fingerprint_covers_dependency_rule_files() {
        let tmp = tempdir().unwrap();
        let rule = make_rule_file(tmp.path(), "rules.txt", b"-keep-global-name\nPlanRule\n");
        let dep_rule = make_rule_file(tmp.path(), "dep-rules.txt", b"-keep-global-name\nFoo\n");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![rule],
            consumer_rule_files: vec![],
        };

        let mut plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        let fp_without_dep = plan.rule_fingerprint();

        plan.rule_inputs[0]
            .dependency_rule_files
            .push(dep_rule.clone());
        let fp_with_dep = plan.rule_fingerprint();

        assert_ne!(
            fp_without_dep, fp_with_dep,
            "fingerprint must change when dependency rule files are added"
        );
    }

    // -- all_rule_files() ------------------------------------------------------

    #[test]
    fn all_rule_files_collects_primary_and_dep_paths() {
        let tmp = tempdir().unwrap();
        let rule = make_rule_file(tmp.path(), "rules.txt", b"-keep-global-name\nPlanRule\n");
        let dep = make_rule_file(tmp.path(), "dep.txt", b"-keep-global-name\nBar\n");
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![rule.clone()],
            consumer_rule_files: vec![],
        };
        let mut plan = ObfuscationPlan::from_build_options(
            &BuildMode::Release,
            Some(&cfg),
            default_output_paths(tmp.path()),
            None,
        );
        plan.rule_inputs[0].dependency_rule_files.push(dep.clone());

        let files = plan.all_rule_files();
        assert!(files.contains(&rule));
        assert!(files.contains(&dep));
        assert_eq!(files.len(), 2);
    }

    // -- ObfuscationRuleInput --------------------------------------------------

    #[test]
    fn rule_input_new_has_no_deps() {
        let p = PathBuf::from("/some/rules.txt");
        let input = ObfuscationRuleInput::new(p.clone());
        assert_eq!(input.rule_file, p);
        assert!(input.dependency_rule_files.is_empty());
    }

    #[test]
    fn rule_input_all_paths_yields_primary_then_deps() {
        let primary = PathBuf::from("/primary.txt");
        let dep1 = PathBuf::from("/dep1.txt");
        let dep2 = PathBuf::from("/dep2.txt");
        let input = ObfuscationRuleInput::with_dependencies(
            primary.clone(),
            vec![dep1.clone(), dep2.clone()],
        );
        let paths: Vec<&PathBuf> = input.all_paths().collect();
        assert_eq!(paths, vec![&primary, &dep1, &dep2]);
    }

    // -- ConsumerObfuscationPlan -----------------------------------------------

    #[test]
    fn consumer_plan_has_consumer_rules_when_non_empty() {
        let plan = ConsumerObfuscationPlan::new(
            vec![PathBuf::from("/consumer.txt")],
            PathBuf::from("/out.txt"),
        );
        assert!(plan.has_consumer_rules());
    }

    #[test]
    fn consumer_plan_no_consumer_rules_when_empty() {
        let plan = ConsumerObfuscationPlan::new(vec![], PathBuf::from("/out.txt"));
        assert!(!plan.has_consumer_rules());
    }

    // -- ObfuscationOutputPaths ------------------------------------------------

    #[test]
    fn output_paths_with_export_rule() {
        let cache = PathBuf::from("/cache");
        let export = PathBuf::from("/export.txt");
        let paths = ObfuscationOutputPaths::with_export_rule(cache.clone(), export.clone());
        assert_eq!(paths.obfuscation_cache_dir, cache);
        assert_eq!(paths.export_rule, Some(export));
    }

    #[test]
    fn output_paths_without_export_rule() {
        let cache = PathBuf::from("/cache");
        let paths = ObfuscationOutputPaths::new(cache.clone());
        assert_eq!(paths.obfuscation_cache_dir, cache);
        assert!(paths.export_rule.is_none());
    }
}
