//! Parser for OHOS-style obfuscation rule files.
//!
//! # Format
//!
//! ```text
//! # comment
//! -enable-property-obfuscation
//! -enable-string-property-obfuscation
//! -enable-toplevel-obfuscation
//! -enable-filename-obfuscation
//! -enable-export-obfuscation
//! -compact
//! -remove-log
//!
//! -keep-property-name
//! propertyToKeep
//! anotherProperty
//!
//! -keep-global-name
//! GlobalClass
//! globalFunction
//! ```

use std::path::Path;

use arkdown_path::{lexical_normalize, AbsPath, ArtifactPathSerializer};
use rustc_hash::FxHashSet;

use crate::ObfuscationError;

/// Parsed, merged set of obfuscation directives.
#[derive(Debug, Clone, Default)]
pub struct ObfuscationRules {
    /// Effective pass enable computed from build-profile data.
    ///
    /// This is not parsed from arkguard rule files; hvigor reads
    /// `buildOption.arkOptions.obfuscation.ruleOptions.enable` from
    /// `build-profile.json5`.
    pub enable_obfuscation: bool,
    pub disable_obfuscation: bool,
    pub enable_property_obfuscation: bool,
    pub enable_string_property_obfuscation: bool,
    pub enable_toplevel_obfuscation: bool,
    pub enable_file_name_obfuscation: bool,
    pub enable_export_obfuscation: bool,
    pub enable_lib_obfuscation_options: bool,
    pub enable_at_keep: bool,
    pub enable_bytecode_obfuscation: bool,
    pub enable_bytecode_obfuscation_debugging: bool,
    pub enable_bytecode_obfuscation_enhanced: bool,
    pub enable_bytecode_obfuscation_arkui: bool,
    pub remove_comments: bool,
    pub compact: bool,
    pub remove_log: bool,
    pub remove_no_side_effect_calls: FxHashSet<String>,
    pub keep_parameter_names: bool,
    pub keep_object_props: bool,
    pub strip_language_default: bool,
    pub strip_system_api_args: bool,
    pub strip_not_compiled_module_name: bool,
    pub print_kept_names: bool,
    pub print_name_cache: Option<String>,
    pub print_kept_names_path: Option<String>,
    pub apply_name_cache: Option<String>,
    pub keep_property_names: FxHashSet<String>,
    pub keep_global_names: FxHashSet<String>,
    pub keep_file_names: FxHashSet<String>,
    pub keep_comments: FxHashSet<String>,
    pub keep_source_paths: FxHashSet<String>,
    pub exclude_source_paths: FxHashSet<String>,
    pub keep_uncompact_paths: FxHashSet<String>,
    pub exclude_uncompact_paths: FxHashSet<String>,
    pub keep_dts_paths: FxHashSet<String>,
    /// Names extracted from `-keep-dts` inputs. ArkGuard stores this same
    /// inventory in `mReservedNames`, `mReservedProperties`, and
    /// `mReservedToplevelNames`; retaining the origin is required because only
    /// `mReservedNames` participates in local-variable preservation.
    pub keep_dts_names: FxHashSet<String>,
}

/// Current `-keep-*` section being accumulated.
enum Section {
    None,
    KeepSource,
    KeepUncompact,
    KeepDts,
    KeepProperty,
    KeepGlobal,
    KeepFile,
    KeepComments,
    RemoveNoSideEffectCalls,
    PrintNameCache,
    PrintKeptNames,
    ApplyNameCache,
    ExtraOptions,
}

impl ObfuscationRules {
    const KEEP: &'static str = "-keep";
    const KEEP_DTS: &'static str = "-keep-dts";
    const KEEP_GLOBAL_NAME: &'static str = "-keep-global-name";
    const KEEP_PROPERTY_NAME: &'static str = "-keep-property-name";
    const KEEP_FILE_NAME: &'static str = "-keep-file-name";
    const KEEP_COMMENTS: &'static str = "-keep-comments";
    const DISABLE_OBFUSCATION: &'static str = "-disable-obfuscation";
    const ENABLE_PROPERTY_OBFUSCATION: &'static str = "-enable-property-obfuscation";
    const ENABLE_STRING_PROPERTY_OBFUSCATION: &'static str = "-enable-string-property-obfuscation";
    const ENABLE_TOPLEVEL_OBFUSCATION: &'static str = "-enable-toplevel-obfuscation";
    const ENABLE_FILENAME_OBFUSCATION: &'static str = "-enable-filename-obfuscation";
    const ENABLE_EXPORT_OBFUSCATION: &'static str = "-enable-export-obfuscation";
    const ENABLE_LIB_OBFUSCATION_OPTIONS: &'static str = "-enable-lib-obfuscation-options";
    const ENABLE_ATKEEP: &'static str = "-use-keep-in-source";
    const REMOVE_COMMENTS: &'static str = "-remove-comments";
    const COMPACT: &'static str = "-compact";
    const REMOVE_LOG: &'static str = "-remove-log";
    const REMOVE_NO_SIDE_EFFECT_CALLS: &'static str = "-remove-nosideeffects-calls";
    const PRINT_NAMECACHE: &'static str = "-print-namecache";
    const PRINT_KEPT_NAMES: &'static str = "-print-kept-names";
    const APPLY_NAMECACHE: &'static str = "-apply-namecache";
    const ENABLE_BYTECODE_OBFUSCATION: &'static str = "-enable-bytecode-obfuscation";
    const ENABLE_BYTECODE_OBFUSCATION_DEBUGGING: &'static str =
        "-enable-bytecode-obfuscation-debugging";
    const ENABLE_BYTECODE_OBFUSCATION_ENHANCED: &'static str =
        "-enable-bytecode-obfuscation-enhanced";
    const ENABLE_BYTECODE_OBFUSCATION_ARKUI: &'static str = "-enable-bytecode-obfuscation-arkui";
    const EXTRA_OPTIONS: &'static str = "-extra-options";
    const STRIP_LANGUAGE_DEFAULT: &'static str = "strip-language-default";
    const STRIP_SYSTEM_API_ARGS: &'static str = "strip-system-api-args";
    const KEEP_PARAMETER_NAMES: &'static str = "-keep-parameter-names";
    const STRIP_NOT_COMPILED_MODULE_NAME: &'static str = "strip-not-compiled-module-name";
    const KEEP_OBJECT_PROPS: &'static str = "-keep-object-props";
    const KEEP_UNCOMPACT: &'static str = "-keep-uncompact";

    /// Parse rule file content from a string.
    pub fn parse(content: &str) -> Self {
        let mut rules = ObfuscationRules::default();
        let mut section = Section::None;
        let uncommented = Self::remove_comments(content);

        for token in ArkguardRuleTokens::new(&uncommented) {
            match token {
                Self::DISABLE_OBFUSCATION => {
                    rules.disable_obfuscation = true;
                    section = Section::None;
                }
                Self::ENABLE_PROPERTY_OBFUSCATION => {
                    rules.enable_property_obfuscation = true;
                    section = Section::None;
                }
                Self::ENABLE_STRING_PROPERTY_OBFUSCATION => {
                    rules.enable_string_property_obfuscation = true;
                    section = Section::None;
                }
                Self::ENABLE_TOPLEVEL_OBFUSCATION => {
                    rules.enable_toplevel_obfuscation = true;
                    section = Section::None;
                }
                Self::ENABLE_FILENAME_OBFUSCATION => {
                    rules.enable_file_name_obfuscation = true;
                    section = Section::None;
                }
                Self::ENABLE_EXPORT_OBFUSCATION => {
                    rules.enable_export_obfuscation = true;
                    section = Section::None;
                }
                Self::ENABLE_LIB_OBFUSCATION_OPTIONS => {
                    rules.enable_lib_obfuscation_options = true;
                    section = Section::None;
                }
                Self::ENABLE_ATKEEP => {
                    rules.enable_at_keep = true;
                    section = Section::None;
                }
                Self::ENABLE_BYTECODE_OBFUSCATION => {
                    rules.enable_bytecode_obfuscation = true;
                    section = Section::None;
                }
                Self::ENABLE_BYTECODE_OBFUSCATION_DEBUGGING => {
                    rules.enable_bytecode_obfuscation_debugging = true;
                    section = Section::None;
                }
                Self::ENABLE_BYTECODE_OBFUSCATION_ENHANCED => {
                    rules.enable_bytecode_obfuscation_enhanced = true;
                    section = Section::None;
                }
                Self::ENABLE_BYTECODE_OBFUSCATION_ARKUI => {
                    rules.enable_bytecode_obfuscation_arkui = true;
                    section = Section::None;
                }
                Self::REMOVE_COMMENTS => {
                    rules.remove_comments = true;
                    section = Section::None;
                }
                Self::COMPACT => {
                    rules.compact = true;
                    section = Section::None;
                }
                Self::REMOVE_LOG => {
                    rules.remove_log = true;
                    section = Section::None;
                }
                Self::KEEP_PARAMETER_NAMES => {
                    rules.keep_parameter_names = true;
                    section = Section::None;
                }
                Self::KEEP_OBJECT_PROPS => {
                    rules.keep_object_props = true;
                    section = Section::None;
                }
                Self::REMOVE_NO_SIDE_EFFECT_CALLS => {
                    section = Section::RemoveNoSideEffectCalls;
                }
                Self::KEEP => {
                    section = Section::KeepSource;
                }
                Self::KEEP_UNCOMPACT => {
                    section = Section::KeepUncompact;
                }
                Self::KEEP_DTS => {
                    section = Section::KeepDts;
                }
                Self::KEEP_PROPERTY_NAME => {
                    section = Section::KeepProperty;
                }
                Self::KEEP_GLOBAL_NAME => {
                    section = Section::KeepGlobal;
                }
                Self::KEEP_FILE_NAME => {
                    section = Section::KeepFile;
                }
                Self::KEEP_COMMENTS => {
                    section = Section::KeepComments;
                }
                Self::PRINT_NAMECACHE => {
                    section = Section::PrintNameCache;
                }
                Self::PRINT_KEPT_NAMES => {
                    rules.print_kept_names = true;
                    section = Section::PrintKeptNames;
                }
                Self::APPLY_NAMECACHE => {
                    section = Section::ApplyNameCache;
                }
                Self::EXTRA_OPTIONS => {
                    section = Section::ExtraOptions;
                }
                name => match section {
                    Section::KeepSource => {
                        rules.keep_source_paths.insert(name.to_string());
                    }
                    Section::KeepUncompact => {
                        rules.keep_uncompact_paths.insert(name.to_string());
                    }
                    Section::KeepDts => {
                        rules.keep_dts_paths.insert(name.to_string());
                    }
                    Section::KeepProperty => {
                        rules.keep_property_names.insert(name.to_string());
                    }
                    Section::KeepGlobal => {
                        rules.keep_global_names.insert(name.to_string());
                    }
                    Section::KeepFile => {
                        rules.keep_file_names.insert(name.to_string());
                    }
                    Section::KeepComments => {
                        rules.keep_comments.insert(name.to_string());
                    }
                    Section::RemoveNoSideEffectCalls => {
                        rules.remove_no_side_effect_calls.insert(name.to_string());
                    }
                    Section::PrintNameCache => {
                        rules.print_name_cache = Some(name.to_string());
                        section = Section::None;
                    }
                    Section::PrintKeptNames => {
                        rules.print_kept_names_path = Some(name.to_string());
                        section = Section::None;
                    }
                    Section::ApplyNameCache => {
                        rules.apply_name_cache = Some(name.to_string());
                        section = Section::None;
                    }
                    Section::ExtraOptions => match name {
                        Self::STRIP_LANGUAGE_DEFAULT => {
                            rules.strip_language_default = true;
                        }
                        Self::STRIP_SYSTEM_API_ARGS => {
                            rules.strip_system_api_args = true;
                        }
                        Self::STRIP_NOT_COMPILED_MODULE_NAME => {
                            rules.strip_not_compiled_module_name = true;
                        }
                        _ => {
                            section = Section::None;
                        }
                    },
                    Section::None => {}
                },
            }
        }
        rules
    }

    /// Parse a rule file from disk.
    pub fn from_file(path: &Path) -> Result<Self, ObfuscationError> {
        let content = std::fs::read_to_string(path)?;
        let mut rules = Self::parse(&content);
        rules.resolve_paths(path)?;
        Ok(rules)
    }

    /// Merge two rule sets (logical OR for flags, union for keep-name sets).
    pub fn merge(self, other: Self) -> Self {
        ObfuscationRules {
            enable_obfuscation: self.enable_obfuscation || other.enable_obfuscation,
            disable_obfuscation: self.disable_obfuscation || other.disable_obfuscation,
            enable_property_obfuscation: self.enable_property_obfuscation
                || other.enable_property_obfuscation,
            enable_string_property_obfuscation: self.enable_string_property_obfuscation
                || other.enable_string_property_obfuscation,
            enable_toplevel_obfuscation: self.enable_toplevel_obfuscation
                || other.enable_toplevel_obfuscation,
            enable_file_name_obfuscation: self.enable_file_name_obfuscation
                || other.enable_file_name_obfuscation,
            enable_export_obfuscation: self.enable_export_obfuscation
                || other.enable_export_obfuscation,
            enable_lib_obfuscation_options: self.enable_lib_obfuscation_options
                || other.enable_lib_obfuscation_options,
            enable_at_keep: self.enable_at_keep || other.enable_at_keep,
            enable_bytecode_obfuscation: self.enable_bytecode_obfuscation
                || other.enable_bytecode_obfuscation,
            enable_bytecode_obfuscation_debugging: self.enable_bytecode_obfuscation_debugging
                || other.enable_bytecode_obfuscation_debugging,
            enable_bytecode_obfuscation_enhanced: self.enable_bytecode_obfuscation_enhanced
                || other.enable_bytecode_obfuscation_enhanced,
            enable_bytecode_obfuscation_arkui: self.enable_bytecode_obfuscation_arkui
                || other.enable_bytecode_obfuscation_arkui,
            remove_comments: self.remove_comments || other.remove_comments,
            compact: self.compact || other.compact,
            remove_log: self.remove_log || other.remove_log,
            remove_no_side_effect_calls: self
                .remove_no_side_effect_calls
                .into_iter()
                .chain(other.remove_no_side_effect_calls)
                .collect(),
            keep_parameter_names: self.keep_parameter_names || other.keep_parameter_names,
            keep_object_props: self.keep_object_props || other.keep_object_props,
            strip_language_default: self.strip_language_default || other.strip_language_default,
            strip_system_api_args: self.strip_system_api_args || other.strip_system_api_args,
            strip_not_compiled_module_name: self.strip_not_compiled_module_name
                || other.strip_not_compiled_module_name,
            print_kept_names: self.print_kept_names || other.print_kept_names,
            print_name_cache: other.print_name_cache.or(self.print_name_cache),
            print_kept_names_path: other.print_kept_names_path.or(self.print_kept_names_path),
            apply_name_cache: other.apply_name_cache.or(self.apply_name_cache),
            keep_property_names: self
                .keep_property_names
                .into_iter()
                .chain(other.keep_property_names)
                .collect(),
            keep_global_names: self
                .keep_global_names
                .into_iter()
                .chain(other.keep_global_names)
                .collect(),
            keep_file_names: self
                .keep_file_names
                .into_iter()
                .chain(other.keep_file_names)
                .collect(),
            keep_comments: self
                .keep_comments
                .into_iter()
                .chain(other.keep_comments)
                .collect(),
            keep_source_paths: self
                .keep_source_paths
                .into_iter()
                .chain(other.keep_source_paths)
                .collect(),
            exclude_source_paths: self
                .exclude_source_paths
                .into_iter()
                .chain(other.exclude_source_paths)
                .collect(),
            keep_uncompact_paths: self
                .keep_uncompact_paths
                .into_iter()
                .chain(other.keep_uncompact_paths)
                .collect(),
            exclude_uncompact_paths: self
                .exclude_uncompact_paths
                .into_iter()
                .chain(other.exclude_uncompact_paths)
                .collect(),
            keep_dts_paths: self
                .keep_dts_paths
                .into_iter()
                .chain(other.keep_dts_paths)
                .collect(),
            keep_dts_names: self
                .keep_dts_names
                .into_iter()
                .chain(other.keep_dts_names)
                .collect(),
        }
    }

    /// Merge only keep-list style dependency rules.
    ///
    /// Mirrors SDK arkguard `MergedConfig.mergeKeepOptions()` used by
    /// `getMergedConfigs()` when `-enable-lib-obfuscation-options` is absent.
    pub fn merge_keep_options(mut self, other: Self) -> Self {
        self.keep_property_names.extend(other.keep_property_names);
        self.keep_global_names.extend(other.keep_global_names);
        self.keep_file_names.extend(other.keep_file_names);
        self.keep_comments.extend(other.keep_comments);
        self.keep_source_paths.extend(other.keep_source_paths);
        self.exclude_source_paths.extend(other.exclude_source_paths);
        self.keep_uncompact_paths.extend(other.keep_uncompact_paths);
        self.exclude_uncompact_paths
            .extend(other.exclude_uncompact_paths);
        self.keep_dts_paths.extend(other.keep_dts_paths);
        self.keep_dts_names.extend(other.keep_dts_names);
        self
    }

    /// Serialize the subset that arkguard exports to HAR `obfuscation.txt`.
    ///
    /// Mirrors `MergedConfig.serializeMergedConfig()` in SDK arkguard
    /// `lib/initialization/ConfigResolver.js`: filename/export/namecache and
    /// keep-comments rules are intentionally not exported.
    pub fn serialize_consumer_config(&self) -> String {
        let mut output = String::new();
        if self.enable_property_obfuscation {
            output.push_str(Self::ENABLE_PROPERTY_OBFUSCATION);
            output.push('\n');
        }
        if self.enable_string_property_obfuscation {
            output.push_str(Self::ENABLE_STRING_PROPERTY_OBFUSCATION);
            output.push('\n');
        }
        if self.enable_toplevel_obfuscation {
            output.push_str(Self::ENABLE_TOPLEVEL_OBFUSCATION);
            output.push('\n');
        }
        if self.compact {
            output.push_str(Self::COMPACT);
            output.push('\n');
        }
        if self.remove_log {
            output.push_str(Self::REMOVE_LOG);
            output.push('\n');
        }
        if self.enable_bytecode_obfuscation_arkui {
            output.push_str(Self::ENABLE_BYTECODE_OBFUSCATION_ARKUI);
            output.push('\n');
        }
        let global_names = SortedRuleNames::from_set(&self.keep_global_names);
        if !global_names.is_empty() {
            output.push_str(Self::KEEP_GLOBAL_NAME);
            output.push('\n');
            global_names.push_lines(&mut output);
        }
        let property_names = SortedRuleNames::from_set(&self.keep_property_names);
        if !property_names.is_empty() {
            output.push_str(Self::KEEP_PROPERTY_NAME);
            output.push('\n');
            property_names.push_lines(&mut output);
        }
        output
    }

    fn remove_comments(content: &str) -> String {
        let mut output = String::with_capacity(content.len());
        let mut in_comment = false;
        for ch in content.chars() {
            if in_comment {
                in_comment = ch != '\n';
            } else if ch == '#' {
                in_comment = true;
            } else {
                output.push(ch);
            }
        }
        output
    }

    /// Resolve path-valued directives while the owning rule file is still
    /// known. SDK `ConfigResolver.resolveKeepConfig` anchors `-keep` entries to
    /// the rule file, canonicalizes existing concrete includes, and keeps
    /// wildcard/exclusion entries for module-inventory matching.
    fn resolve_paths(&mut self, config_path: &Path) -> Result<(), ObfuscationError> {
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
        let configured_paths = std::mem::take(&mut self.keep_source_paths);
        for configured in configured_paths {
            let (exclude, value) = configured
                .strip_prefix('!')
                .map_or((false, configured.as_str()), |value| (true, value));
            let unresolved = Path::new(value);
            let anchored = if unresolved.is_absolute() {
                unresolved.to_path_buf()
            } else {
                config_dir.join(unresolved)
            };
            let resolved = lexical_normalize(&anchored);
            let contains_wildcards = value.contains(['*', '?']);
            if !exclude && !contains_wildcards && !resolved.exists() {
                // ArkGuard reports a warning and ignores a missing concrete
                // include. Missing wildcard matches and exclusions are valid.
                continue;
            }
            let resolved = if !contains_wildcards && resolved.exists() {
                AbsPath::canonicalize(&resolved)?.into_path_buf()
            } else {
                resolved
            };
            let serialized = ArtifactPathSerializer::path_to_slash_string(&resolved)?;
            if exclude {
                self.exclude_source_paths.insert(serialized);
            } else {
                self.keep_source_paths.insert(serialized);
            }
        }

        let configured_paths = std::mem::take(&mut self.keep_uncompact_paths);
        for configured in configured_paths {
            let (exclude, value) = configured
                .strip_prefix('!')
                .map_or((false, configured.as_str()), |value| (true, value));
            let unresolved = Path::new(value);
            let anchored = if unresolved.is_absolute() {
                unresolved.to_path_buf()
            } else {
                config_dir.join(unresolved)
            };
            let resolved = lexical_normalize(&anchored);
            let contains_wildcards = value.contains(['*', '?']);
            if !exclude && !contains_wildcards && !resolved.exists() {
                // `ConfigResolver.resolveKeepUncompactConfig` warns and drops
                // concrete paths which do not exist.
                continue;
            }
            let resolved = if !contains_wildcards && resolved.exists() {
                AbsPath::canonicalize(&resolved)?.into_path_buf()
            } else {
                resolved
            };
            let serialized = ArtifactPathSerializer::path_to_slash_string(&resolved)?;
            if exclude {
                self.exclude_uncompact_paths.insert(serialized);
            } else {
                self.keep_uncompact_paths.insert(serialized);
            }
        }

        let dts_paths = std::mem::take(&mut self.keep_dts_paths);
        for configured in dts_paths {
            let path = Path::new(&configured);
            let anchored = if path.is_absolute() {
                path.to_path_buf()
            } else {
                config_dir.join(path)
            };
            let resolved = lexical_normalize(&anchored);
            self.keep_dts_paths
                .insert(ArtifactPathSerializer::path_to_slash_string(&resolved)?);
        }

        for configured in [
            &mut self.print_name_cache,
            &mut self.print_kept_names_path,
            &mut self.apply_name_cache,
        ] {
            let Some(value) = configured.as_deref() else {
                continue;
            };
            let path = Path::new(value);
            let anchored = if path.is_absolute() {
                path.to_path_buf()
            } else {
                config_dir.join(path)
            };
            let resolved = lexical_normalize(&anchored);
            *configured = Some(ArtifactPathSerializer::path_to_slash_string(&resolved)?);
        }
        Ok(())
    }
}

struct ArkguardRuleTokens<'a> {
    tokens: Vec<&'a str>,
}

impl<'a> ArkguardRuleTokens<'a> {
    fn new(content: &'a str) -> Self {
        let mut ordinary = Vec::new();
        let mut remove_calls = Vec::new();
        let mut cursor = 0;
        while let Some(offset) =
            content[cursor..].find(ObfuscationRules::REMOVE_NO_SIDE_EFFECT_CALLS)
        {
            let start = cursor + offset;
            ordinary.push(&content[cursor..start]);
            let value_start = start + ObfuscationRules::REMOVE_NO_SIDE_EFFECT_CALLS.len();
            let mut end = Self::nearest_option(content, value_start).unwrap_or(content.len());
            while end > value_start {
                let Some(last) = content[..end].chars().next_back() else {
                    break;
                };
                if !Self::is_ordinary_delimiter(last) {
                    break;
                }
                end -= last.len_utf8();
            }
            remove_calls.push(&content[start..end]);
            cursor = end;
        }
        ordinary.push(&content[cursor..]);

        let mut tokens = ordinary
            .into_iter()
            .flat_map(|part| part.split(Self::is_ordinary_delimiter))
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>();
        tokens.extend(
            remove_calls
                .into_iter()
                .flat_map(|part| part.split(Self::is_remove_call_delimiter))
                .filter(|token| !token.is_empty()),
        );
        Self { tokens }
    }

    fn nearest_option(content: &str, start: usize) -> Option<usize> {
        const OPTIONS: &[&str] = &[
            ObfuscationRules::KEEP,
            ObfuscationRules::KEEP_DTS,
            ObfuscationRules::KEEP_GLOBAL_NAME,
            ObfuscationRules::KEEP_PROPERTY_NAME,
            ObfuscationRules::KEEP_FILE_NAME,
            ObfuscationRules::KEEP_COMMENTS,
            ObfuscationRules::DISABLE_OBFUSCATION,
            ObfuscationRules::ENABLE_PROPERTY_OBFUSCATION,
            ObfuscationRules::ENABLE_STRING_PROPERTY_OBFUSCATION,
            ObfuscationRules::ENABLE_TOPLEVEL_OBFUSCATION,
            ObfuscationRules::ENABLE_FILENAME_OBFUSCATION,
            ObfuscationRules::ENABLE_EXPORT_OBFUSCATION,
            ObfuscationRules::ENABLE_LIB_OBFUSCATION_OPTIONS,
            ObfuscationRules::ENABLE_ATKEEP,
            ObfuscationRules::REMOVE_COMMENTS,
            ObfuscationRules::COMPACT,
            ObfuscationRules::REMOVE_LOG,
            ObfuscationRules::REMOVE_NO_SIDE_EFFECT_CALLS,
            ObfuscationRules::PRINT_NAMECACHE,
            ObfuscationRules::PRINT_KEPT_NAMES,
            ObfuscationRules::APPLY_NAMECACHE,
            ObfuscationRules::ENABLE_BYTECODE_OBFUSCATION,
            ObfuscationRules::ENABLE_BYTECODE_OBFUSCATION_DEBUGGING,
            ObfuscationRules::ENABLE_BYTECODE_OBFUSCATION_ENHANCED,
            ObfuscationRules::ENABLE_BYTECODE_OBFUSCATION_ARKUI,
            ObfuscationRules::EXTRA_OPTIONS,
            ObfuscationRules::STRIP_LANGUAGE_DEFAULT,
            ObfuscationRules::STRIP_SYSTEM_API_ARGS,
            ObfuscationRules::KEEP_PARAMETER_NAMES,
            ObfuscationRules::STRIP_NOT_COMPILED_MODULE_NAME,
            ObfuscationRules::KEEP_OBJECT_PROPS,
            ObfuscationRules::KEEP_UNCOMPACT,
        ];
        OPTIONS
            .iter()
            .filter_map(|option| content[start..].find(option).map(|index| start + index))
            .min()
    }

    fn is_ordinary_delimiter(ch: char) -> bool {
        matches!(ch, '\'' | ',' | '\t' | ' ' | '\n' | '\r')
    }

    fn is_remove_call_delimiter(ch: char) -> bool {
        matches!(ch, ',' | '\t' | ' ' | '\n' | '\r')
    }
}

impl<'a> IntoIterator for ArkguardRuleTokens<'a> {
    type Item = &'a str;
    type IntoIter = std::vec::IntoIter<&'a str>;

    fn into_iter(self) -> Self::IntoIter {
        self.tokens.into_iter()
    }
}

struct SortedRuleNames {
    names: Vec<String>,
}

impl SortedRuleNames {
    fn from_set(names: &FxHashSet<String>) -> Self {
        let mut names: Vec<String> = names.iter().cloned().collect();
        names.sort();
        names.dedup();
        Self { names }
    }

    fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    fn push_lines(&self, output: &mut String) {
        for name in &self.names {
            output.push_str(name);
            output.push('\n');
        }
    }
}
