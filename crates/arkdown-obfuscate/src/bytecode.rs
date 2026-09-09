use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use arkdown_path::ArtifactPathSerializer;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AccessorProperty, ArrowFunctionExpression, BindingIdentifier, Class, ClassElement,
    ComputedMemberExpression, Decorator, ExportDeclaration, ExportDefaultDeclaration,
    ExportNamedDeclaration, ExportSpecifier, Expression, FormalParameter, FormalParameterRest,
    Function, IdentifierName, IdentifierReference, MethodDefinition, ModuleExportName,
    ObjectProperty, PropertyDefinition, PropertyKey, StringLiteral, StructElement, StructStatement,
    TSEnumDeclaration, TSEnumMemberName, TSIndexSignatureName, TSInterfaceDeclaration, TSSignature,
    TSTypeParameter, VariableDeclaration,
};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use rayon::prelude::*;
use rolldown_sourcemap::OwnedSourceMap;
use rustc_hash::FxHashSet;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{
    ArkguardAdapterOptions, ObfuscationError, ObfuscationModuleFile, ObfuscationModuleInput,
    ObfuscationOptions, ObfuscationRules, ObfuscationSource, ObfuscationSourceKind,
};

const ES_ANNOTATIONS: [&str; 3] = [
    "_ESConcurrentModuleRequestsAnnotation",
    "_ESSlotNumberAnnotation",
    "_ESAnnotation",
];
const BYTECODE_PROPERTY_WHITE_LIST: [&str; 3] =
    ["componentClass", "getReuseId", "resetStateVarsOnReuse"];
const NAME_CACHE_METADATA_KEYS: [&str; 5] = [
    "FileNameCache",
    "ObfNameMap",
    "ProPertyCache",
    "entryPackageInfo",
    "compileSdkVersion",
];

/// Original source path to ABC record mapping used by SDK keep-rule handling.
#[derive(Debug, Clone)]
pub struct BytecodeObfuscationModuleRecord {
    pub source_path: PathBuf,
    pub record_name: String,
}

/// Inputs that become available only after `es2abc` completes.
#[derive(Debug, Clone)]
pub struct BytecodeObfuscationInput {
    pub module: ObfuscationModuleInput,
    pub modules_abc_path: PathBuf,
    pub module_records: Vec<BytecodeObfuscationModuleRecord>,
    pub record_name_white_list: Vec<String>,
    pub reserved_name_sources: Vec<ObfuscationSource>,
}

/// SDK bytecode-obfuscation artifacts retained in the CompileArkTS cache.
#[derive(Debug, Clone)]
pub struct BytecodeObfuscationOutput {
    pub modules_abc_path: PathBuf,
    pub config_path: PathBuf,
    pub name_cache_path: PathBuf,
    pub source_maps_path: PathBuf,
    pub origin_abc_path: PathBuf,
    pub obfuscated_abc_path: PathBuf,
}

pub(crate) struct ArkguardBytecodeRunner<'a> {
    options: &'a ObfuscationOptions,
    rules: &'a ObfuscationRules,
    adapter: &'a ArkguardAdapterOptions,
}

impl<'a> ArkguardBytecodeRunner<'a> {
    pub(crate) fn new(
        options: &'a ObfuscationOptions,
        rules: &'a ObfuscationRules,
        adapter: &'a ArkguardAdapterOptions,
    ) -> Self {
        Self {
            options,
            rules,
            adapter,
        }
    }

    pub(crate) fn run(
        &self,
        input: BytecodeObfuscationInput,
    ) -> Result<BytecodeObfuscationOutput, ObfuscationError> {
        self.validate(&input)?;
        let paths = BytecodeObfuscationPaths::new(self.options, self.rules, self.adapter);
        paths.create_directories()?;
        paths.stage_source_maps(&input)?;

        let inventory = ArkguardReservedNameInventory::build(self.adapter, self.rules, &input)?;
        let config = BytecodeObfuscationConfigBuilder::new(
            self.options,
            self.rules,
            self.adapter,
            &input,
            &paths,
            inventory,
        )
        .build()?;
        fs_err::write(&paths.config, serde_json::to_vec_pretty(&config)?)?;
        fs_err::copy(&input.modules_abc_path, &paths.origin_abc)?;

        let mut command = Command::new(&self.adapter.panda_guard_path);
        if self.rules.enable_bytecode_obfuscation_debugging {
            command
                .arg("--debug")
                .arg("--debug-file")
                .arg(&paths.debug_log);
            if !paths.debug_log.exists() {
                fs_err::write(&paths.debug_log, [])?;
            }
        }
        let process = command.arg(&paths.config).output().map_err(|source| {
            ObfuscationError::PandaGuardProcessStart {
                executable: self.adapter.panda_guard_path.clone(),
                source,
            }
        })?;
        if !process.status.success() {
            return Err(ObfuscationError::PandaGuardProcessFailed {
                status: process.status.code(),
                stderr: String::from_utf8_lossy(&process.stderr).trim().to_string(),
            });
        }
        if !paths.obfuscated_abc.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationOutputNotFound(
                paths.obfuscated_abc,
            ));
        }
        fs_err::copy(&paths.obfuscated_abc, &input.modules_abc_path)?;
        self.update_post_obfuscation_artifacts(&paths, self.rules.compact)?;
        paths.publish_source_maps(&input)?;
        self.validate_outputs(&paths)?;

        Ok(BytecodeObfuscationOutput {
            modules_abc_path: input.modules_abc_path,
            config_path: paths.config,
            name_cache_path: paths.name_cache,
            source_maps_path: paths.source_maps,
            origin_abc_path: paths.origin_abc,
            obfuscated_abc_path: paths.obfuscated_abc,
        })
    }

    fn validate(&self, input: &BytecodeObfuscationInput) -> Result<(), ObfuscationError> {
        if !self.adapter.panda_guard_path.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationInputNotFound(
                self.adapter.panda_guard_path.clone(),
            ));
        }
        if self.adapter.compatible_sdk_version < 13
            && !(self.adapter.compatible_sdk_version == 12
                && self.adapter.compatible_sdk_version_stage.as_deref() == Some("beta3"))
        {
            return Err(
                ObfuscationError::BytecodeObfuscationUnsupportedCompatibleSdk {
                    version: self.adapter.compatible_sdk_version,
                    stage: self.adapter.compatible_sdk_version_stage.clone(),
                },
            );
        }
        if !input.modules_abc_path.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationInputNotFound(
                input.modules_abc_path.clone(),
            ));
        }
        let expected_abc = self.adapter.ace_module_build.join("modules.abc");
        if input.modules_abc_path != expected_abc {
            return Err(ObfuscationError::BytecodeObfuscationAbcPathMismatch {
                expected: expected_abc,
                actual: input.modules_abc_path.clone(),
            });
        }
        let source_maps = input
            .module
            .source_maps_path
            .as_ref()
            .ok_or(ObfuscationError::BytecodeObfuscationSourceMapsRequired)?;
        if !source_maps.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationInputNotFound(
                source_maps.clone(),
            ));
        }
        let files_info = self.adapter.cache_path.join("filesInfo.txt");
        if !files_info.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationInputNotFound(
                files_info,
            ));
        }
        Ok(())
    }

    fn update_post_obfuscation_artifacts(
        &self,
        paths: &BytecodeObfuscationPaths,
        compact: bool,
    ) -> Result<(), ObfuscationError> {
        if !paths.name_cache.is_file() {
            return Err(ObfuscationError::BytecodeObfuscationOutputNotFound(
                paths.name_cache.clone(),
            ));
        }
        let mut name_cache: Map<String, Value> =
            serde_json::from_slice(&fs_err::read(&paths.name_cache)?)?;
        let original_source_maps: Map<String, Value> =
            serde_json::from_slice(&fs_err::read(&paths.source_maps)?)?;
        let cached_source_maps = self.adapter.cache_path.join("sourceMaps.json");
        if cached_source_maps.is_file() {
            fs_err::copy(&cached_source_maps, &paths.backup_source_maps)?;
        }

        let file_mapping = NameCacheProcessor::source_file_mapping(&name_cache);
        let updated_source_maps =
            NameCacheProcessor::rename_source_map_keys(original_source_maps.clone(), &file_mapping);
        fs_err::write(
            &paths.source_maps,
            serde_json::to_vec_pretty(&updated_source_maps)?,
        )?;
        fs_err::copy(&paths.source_maps, &cached_source_maps)?;
        if !compact {
            name_cache = NameCacheProcessor::remap_line_ranges(name_cache, &original_source_maps)?;
            fs_err::write(&paths.name_cache, serde_json::to_vec_pretty(&name_cache)?)?;
        }
        Ok(())
    }

    fn validate_outputs(&self, paths: &BytecodeObfuscationPaths) -> Result<(), ObfuscationError> {
        for path in [
            &paths.config,
            &paths.name_cache,
            &paths.source_maps,
            &paths.origin_abc,
            &paths.obfuscated_abc,
        ] {
            if !path.is_file() {
                return Err(ObfuscationError::BytecodeObfuscationOutputNotFound(
                    path.clone(),
                ));
            }
        }
        Ok(())
    }
}

struct BytecodeObfuscationPaths {
    config: PathBuf,
    default_name_cache: PathBuf,
    name_cache: PathBuf,
    source_maps: PathBuf,
    backup_source_maps: PathBuf,
    origin_abc: PathBuf,
    obfuscated_abc: PathBuf,
    obfuscated_pa: Option<PathBuf>,
    debug_log: PathBuf,
}

impl BytecodeObfuscationPaths {
    fn new(
        options: &ObfuscationOptions,
        rules: &ObfuscationRules,
        adapter: &ArkguardAdapterOptions,
    ) -> Self {
        let origin = options.cache_dir.join("origin");
        let obfuscated = options.cache_dir.join("obf");
        let default_name_cache = options.cache_dir.join("nameCache.json");
        let name_cache = RuleOutputPathResolver::new(options)
            .resolve(rules.print_name_cache.as_deref())
            .unwrap_or_else(|| default_name_cache.clone());
        let obfuscated_abc = obfuscated.join("modules.abc");
        Self {
            config: options.cache_dir.join("config.json"),
            default_name_cache,
            name_cache,
            source_maps: adapter.cache_path.join("sourceMaps.map"),
            backup_source_maps: origin.join("sourceMaps.json"),
            origin_abc: origin.join("modules.abc"),
            obfuscated_pa: rules
                .enable_bytecode_obfuscation_debugging
                .then(|| obfuscated_abc.with_extension("pa")),
            obfuscated_abc,
            debug_log: options.cache_dir.join("debug.log"),
        }
    }

    fn create_directories(&self) -> Result<(), std::io::Error> {
        for path in [&self.config, &self.origin_abc, &self.obfuscated_abc] {
            if let Some(parent) = path.parent() {
                fs_err::create_dir_all(parent)?;
            }
        }
        Ok(())
    }

    fn stage_source_maps(&self, input: &BytecodeObfuscationInput) -> Result<(), ObfuscationError> {
        let source_maps = input
            .module
            .source_maps_path
            .as_ref()
            .ok_or(ObfuscationError::BytecodeObfuscationSourceMapsRequired)?;
        if source_maps != &self.source_maps {
            if let Some(parent) = self.source_maps.parent() {
                fs_err::create_dir_all(parent)?;
            }
            fs_err::copy(source_maps, &self.source_maps)?;
        }
        Ok(())
    }

    fn publish_source_maps(
        &self,
        input: &BytecodeObfuscationInput,
    ) -> Result<(), ObfuscationError> {
        let source_maps = input
            .module
            .source_maps_path
            .as_ref()
            .ok_or(ObfuscationError::BytecodeObfuscationSourceMapsRequired)?;
        if source_maps != &self.source_maps {
            fs_err::copy(&self.source_maps, source_maps)?;
        }
        Ok(())
    }
}

struct RuleOutputPathResolver<'a> {
    options: &'a ObfuscationOptions,
}

impl<'a> RuleOutputPathResolver<'a> {
    fn new(options: &'a ObfuscationOptions) -> Self {
        Self { options }
    }

    fn resolve(&self, value: Option<&str>) -> Option<PathBuf> {
        let path = Path::new(value?);
        Some(if path.has_root() {
            path.to_path_buf()
        } else {
            self.options
                .rule_files
                .first()
                .and_then(|rule| rule.parent())
                .unwrap_or(Path::new("."))
                .join(path)
        })
    }

    fn serialized(&self, value: Option<&str>) -> Result<String, ObfuscationError> {
        self.resolve(value)
            .as_deref()
            .map(ArtifactPathSerializer::path_to_slash_string)
            .transpose()
            .map(|path| path.unwrap_or_default())
            .map_err(Into::into)
    }
}

#[derive(Default)]
pub(crate) struct ArkguardReservedNameReasons {
    pub(crate) sdk_properties: BTreeSet<String>,
    pub(crate) sdk_globals: BTreeSet<String>,
    pub(crate) sdk_locals: BTreeSet<String>,
    pub(crate) language_properties: BTreeSet<String>,
    pub(crate) configured_properties: BTreeSet<String>,
    pub(crate) configured_globals: BTreeSet<String>,
    pub(crate) configured_locals: BTreeSet<String>,
    pub(crate) atkeep_properties: BTreeSet<String>,
    pub(crate) atkeep_globals: BTreeSet<String>,
    pub(crate) intent_properties: BTreeSet<String>,
    pub(crate) intent_globals: BTreeSet<String>,
    pub(crate) structs: BTreeSet<String>,
    pub(crate) strings: BTreeSet<String>,
    pub(crate) objects: BTreeSet<String>,
    pub(crate) export_properties: BTreeSet<String>,
    pub(crate) export_globals: BTreeSet<String>,
    pub(crate) enums: BTreeSet<String>,
}

pub(crate) struct ArkguardReservedNameInventory {
    pub(crate) names: BTreeSet<String>,
    pub(crate) reasons: ArkguardReservedNameReasons,
}

impl ArkguardReservedNameInventory {
    fn build(
        adapter: &ArkguardAdapterOptions,
        rules: &ObfuscationRules,
        input: &BytecodeObfuscationInput,
    ) -> Result<Self, ObfuscationError> {
        let keep_plan = input.module.keep_plan(rules, adapter);
        Self::build_for_sources(
            adapter,
            rules,
            &input.reserved_name_sources,
            &input.module.files,
            &keep_plan.export_dependency_paths,
        )
    }

    pub(crate) fn build_for_sources(
        adapter: &ArkguardAdapterOptions,
        rules: &ObfuscationRules,
        sources: &[ObfuscationSource],
        files: &[ObfuscationModuleFile],
        preserve_exports: &rustc_hash::FxHashSet<PathBuf>,
    ) -> Result<Self, ObfuscationError> {
        let scans = sources
            .par_iter()
            .map(|source| {
                ReservedNameSourceScanner::new(
                    source,
                    rules,
                    preserve_exports.contains(&source.path),
                    adapter.compile_har,
                )
                .scan()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut names = BTreeSet::new();
        let mut reasons = ArkguardReservedNameReasons::default();
        for scan in scans {
            names.extend(scan.names);
            reasons.sdk_properties.extend(scan.sdk_properties);
            reasons.sdk_globals.extend(scan.sdk_globals);
            reasons.sdk_locals.extend(scan.sdk_locals);
            reasons.structs.extend(scan.structs);
            reasons.strings.extend(scan.strings);
            reasons.objects.extend(scan.objects);
            reasons.export_properties.extend(scan.export_properties);
            reasons.export_globals.extend(scan.export_globals);
            reasons.enums.extend(scan.enums);
            reasons.atkeep_properties.extend(scan.configured_properties);
            reasons
                .atkeep_properties
                .extend(scan.configured_globals.iter().cloned());
            reasons.atkeep_globals.extend(scan.configured_globals);
        }
        reasons.configured_properties.extend(
            rules
                .keep_property_names
                .iter()
                .chain(rules.keep_global_names.iter())
                .filter(|name| !name.contains(['*', '?']))
                .cloned(),
        );
        reasons.configured_globals.extend(
            rules
                .keep_global_names
                .iter()
                .filter(|name| !name.contains(['*', '?']))
                .cloned(),
        );
        reasons
            .configured_locals
            .extend(rules.keep_dts_names.iter().cloned());
        names.extend(reasons.configured_properties.iter().cloned());
        names.extend(reasons.configured_globals.iter().cloned());
        names.extend(reasons.configured_locals.iter().cloned());
        names.extend(ES_ANNOTATIONS.into_iter().map(str::to_string));
        for file in files {
            names.extend(file.insight_intent_global_names.iter().cloned());
            names.extend(file.insight_intent_property_names.iter().cloned());
            reasons
                .intent_globals
                .extend(file.insight_intent_global_names.iter().cloned());
            reasons
                .intent_properties
                .extend(file.insight_intent_global_names.iter().cloned());
            reasons
                .intent_properties
                .extend(file.insight_intent_property_names.iter().cloned());
        }
        let language = Self::language_reserved_names(adapter, rules)?;
        names.extend(language.iter().cloned());
        reasons.language_properties = language;
        reasons
            .sdk_properties
            .extend(adapter.arkui_reserved_property_names.iter().cloned());
        if rules.strip_system_api_args {
            reasons.sdk_locals.clear();
            reasons.sdk_properties.extend(
                adapter
                    .arkui_optimized_reserved_property_names
                    .iter()
                    .cloned(),
            );
        }
        Ok(Self { names, reasons })
    }

    pub(crate) fn collect_keep_dts_names(
        paths: &rustc_hash::FxHashSet<String>,
        rules: &ObfuscationRules,
    ) -> Result<BTreeSet<String>, ObfuscationError> {
        let mut files = Vec::new();
        for configured in paths {
            let path = PathBuf::from(configured);
            if !path.exists() {
                return Err(ObfuscationError::KeepDtsNotFound(path));
            }
            if path.is_file() {
                files.push(path);
                continue;
            }
            for entry in walkdir::WalkDir::new(&path).follow_links(false) {
                let entry = entry.map_err(|error| {
                    error.into_io_error().map_or_else(
                        || {
                            ObfuscationError::CodegenError(format!(
                                "failed to traverse ArkGuard -keep-dts path {}",
                                path.display()
                            ))
                        },
                        ObfuscationError::Io,
                    )
                })?;
                if entry.file_type().is_file()
                    && matches!(
                        entry
                            .path()
                            .extension()
                            .and_then(|extension| extension.to_str()),
                        Some("ets" | "ts" | "js")
                    )
                {
                    files.push(entry.into_path());
                }
            }
        }
        files.sort();
        files.dedup();
        let sources = files
            .par_iter()
            .map(|path| {
                fs_err::read_to_string(path).map(|source| ObfuscationSource {
                    path: path.clone(),
                    source: source.into(),
                    kind: ObfuscationSourceKind::KeepDts,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scans = sources
            .par_iter()
            .map(|source| ReservedNameSourceScanner::new(source, rules, false, false).scan())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(scans.into_iter().flat_map(|scan| scan.names).collect())
    }

    fn language_reserved_names(
        adapter: &ArkguardAdapterOptions,
        rules: &ObfuscationRules,
    ) -> Result<BTreeSet<String>, ObfuscationError> {
        let path = adapter.arkguard_package.join("lib/configs/preset").join(
            if rules.strip_language_default {
                "es_reserved_properties_optimized.json"
            } else {
                "es_reserved_properties.json"
            },
        );
        let source = fs_err::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ObfuscationError::ArkguardPackageNotFound(path.clone())
            } else {
                ObfuscationError::Io(error)
            }
        })?;
        let value: Value = serde_json::from_slice(&source)?;
        let mut names = BTreeSet::new();
        if let Some(groups) = value.as_object() {
            for values in groups.values().filter_map(Value::as_array) {
                names.extend(values.iter().filter_map(Value::as_str).map(str::to_string));
            }
        }
        Ok(names)
    }
}

struct ReservedNameSourceScanner<'a> {
    source: &'a ObfuscationSource,
    rules: &'a ObfuscationRules,
    preserve_exports: bool,
    compile_har: bool,
}

impl<'a> ReservedNameSourceScanner<'a> {
    fn new(
        source: &'a ObfuscationSource,
        rules: &'a ObfuscationRules,
        preserve_exports: bool,
        compile_har: bool,
    ) -> Self {
        Self {
            source,
            rules,
            preserve_exports,
            compile_har,
        }
    }

    fn scan(&self) -> Result<ReservedNameSourceScan, ObfuscationError> {
        if self.source.kind == ObfuscationSourceKind::PreDefine {
            if self.rules.strip_system_api_args {
                return Ok(ReservedNameSourceScan::default());
            }
            let sdk = Self::collect_pre_define_strings(&self.source.source);
            return Ok(ReservedNameSourceScan {
                names: sdk.clone(),
                sdk_properties: sdk.clone(),
                sdk_locals: sdk,
                ..ReservedNameSourceScan::default()
            });
        }
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &self.source.source, self.source_type()).parse();
        // `ApiExtractor.parseFile` uses TypeScript's recovery parser and does
        // not reject declaration diagnostics while building ArkGuard
        // whitelists. OXC likewise returns a usable recovery AST here; source
        // compilation owns diagnostics separately.
        let is_declaration = self
            .source
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".d.ts") || name.ends_with(".d.ets"));
        let mut collector = ReservedNameAstCollector::new(
            self.source.kind,
            matches!(
                self.source.kind,
                ObfuscationSourceKind::SdkComponent | ObfuscationSourceKind::KeepDts
            ) || (self.source.kind == ObfuscationSourceKind::Project
                && is_declaration
                && (!self.rules.enable_export_obfuscation || self.preserve_exports)),
            !self.rules.enable_string_property_obfuscation,
            self.rules.keep_object_props,
            !self.rules.enable_export_obfuscation || self.preserve_exports,
            self.compile_har,
            self.rules.strip_system_api_args,
        );
        collector.visit_program(&parsed.program);
        if self.rules.enable_at_keep && self.supports_at_keep() {
            let configured = AtKeepNameCollector::collect(&parsed.program);
            collector
                .scan
                .names
                .extend(configured.globals.iter().cloned());
            collector
                .scan
                .names
                .extend(configured.properties.iter().cloned());
            collector
                .scan
                .configured_globals
                .extend(configured.globals.iter().cloned());
            collector
                .scan
                .configured_properties
                .extend(configured.properties);
        }
        Ok(collector.scan)
    }

    /// Mirrors `ApiExtractor.extractStringsFromFile`'s
    /// `/"([^"]*)"/g` collection without treating the loader implementation's
    /// identifiers as SDK API names.
    fn collect_pre_define_strings(source: &str) -> BTreeSet<String> {
        static DOUBLE_QUOTED: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new(r#""([^"]*)""#).expect("valid regex"));
        DOUBLE_QUOTED
            .captures_iter(source)
            .filter_map(|capture| capture.get(1).map(|value| value.as_str().to_string()))
            .collect()
    }

    fn supports_at_keep(&self) -> bool {
        let Some(name) = self.source.path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        !name.ends_with(".d.ts")
            && !name.ends_with(".d.ets")
            && (name.ends_with(".ts") || name.ends_with(".ets"))
    }

    fn source_type(&self) -> SourceType {
        match self
            .source
            .path
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("ets") | Some("ts") => SourceType::ets(),
            _ => SourceType::from_path(&self.source.path)
                .unwrap_or_else(|_| SourceType::ts().with_module(true)),
        }
    }
}

#[derive(Default)]
struct ReservedNameSourceScan {
    names: BTreeSet<String>,
    sdk_properties: BTreeSet<String>,
    sdk_globals: BTreeSet<String>,
    sdk_locals: BTreeSet<String>,
    structs: BTreeSet<String>,
    strings: BTreeSet<String>,
    objects: BTreeSet<String>,
    export_properties: BTreeSet<String>,
    export_globals: BTreeSet<String>,
    enums: BTreeSet<String>,
    configured_properties: BTreeSet<String>,
    configured_globals: BTreeSet<String>,
}

#[derive(Default)]
struct AtKeepNames {
    globals: BTreeSet<String>,
    properties: BTreeSet<String>,
}

struct AtKeepContainer {
    name: Option<String>,
    global: bool,
    exported: bool,
}

struct AtKeepNameCollector<'source> {
    source: &'source str,
    targets: FxHashSet<u32>,
    names: AtKeepNames,
    containers: Vec<AtKeepContainer>,
    declaration_depth: usize,
    export_depth: usize,
}

impl<'source> AtKeepNameCollector<'source> {
    fn collect(program: &'source oxc_ast::ast::Program<'source>) -> AtKeepNames {
        let targets: FxHashSet<u32> = program
            .comments
            .iter()
            .filter(|comment| comment.is_line())
            .filter_map(|comment| {
                let span = comment.content_span();
                let content = program
                    .source_text
                    .get(span.start as usize..span.end as usize)?;
                let directive = content
                    .chars()
                    .filter(|ch| !ch.is_whitespace())
                    .collect::<String>();
                matches!(directive.as_str(), "@KeepSymbol" | "@KeepAsConsumer")
                    .then_some(comment.attached_to)
            })
            .collect();
        if targets.is_empty() {
            return AtKeepNames::default();
        }
        let mut collector = Self {
            source: program.source_text,
            targets,
            names: AtKeepNames::default(),
            containers: Vec::new(),
            declaration_depth: 0,
            export_depth: 0,
        };
        collector.visit_program(program);
        collector.names
    }

    fn is_tagged(&self, start: u32) -> bool {
        if self.targets.contains(&start) {
            return true;
        }
        self.targets.iter().any(|target| {
            if *target >= start {
                return false;
            }
            self.source
                .get(*target as usize..start as usize)
                .is_some_and(|prefix| {
                    prefix.split_whitespace().all(|token| {
                        matches!(
                            token,
                            "export" | "default" | "declare" | "abstract" | "async"
                        )
                    })
                })
        })
    }

    fn insert_key(&mut self, key: &PropertyKey<'_>) {
        match key {
            PropertyKey::StaticIdentifier(identifier) => {
                self.names.properties.insert(identifier.name.to_string());
            }
            PropertyKey::StringLiteral(literal) => {
                self.names.properties.insert(literal.value.to_string());
            }
            PropertyKey::PrivateIdentifier(identifier) => {
                self.names.properties.insert(identifier.name.to_string());
            }
            _ => {}
        }
    }

    fn insert_class_member(&mut self, member: &ClassElement<'_>) {
        match member {
            ClassElement::MethodDefinition(method) => self.insert_key(&method.key),
            ClassElement::PropertyDefinition(property) => self.insert_key(&property.key),
            ClassElement::AccessorProperty(property) => self.insert_key(&property.key),
            ClassElement::ETSOverloadDeclaration(overload) => self.insert_key(&overload.key),
            ClassElement::StaticBlock(_)
            | ClassElement::TSIndexSignature(_)
            | ClassElement::TSCallSignatureDeclaration(_) => {}
        }
    }

    fn insert_interface_member(&mut self, member: &TSSignature<'_>) {
        match member {
            TSSignature::TSPropertySignature(property) => self.insert_key(&property.key),
            TSSignature::TSMethodSignature(method) => self.insert_key(&method.key),
            TSSignature::MethodDefinition(method) => self.insert_key(&method.key),
            TSSignature::PropertyDefinition(property) => self.insert_key(&property.key),
            TSSignature::ETSOverloadDeclaration(overload) => self.insert_key(&overload.key),
            TSSignature::TSIndexSignature(_)
            | TSSignature::TSCallSignatureDeclaration(_)
            | TSSignature::TSConstructSignatureDeclaration(_) => {}
        }
    }

    fn insert_declaration_name(&mut self, name: Option<&str>) {
        let Some(name) = name else {
            return;
        };
        if self.declaration_depth == 0 || self.export_depth > 0 {
            self.names.globals.insert(name.to_string());
        }
        if self.export_depth > 0 {
            self.names.properties.insert(name.to_string());
        }
    }

    fn insert_current_container_name(&mut self) {
        if let Some(container) = self.containers.last() {
            if let Some(name) = &container.name {
                if container.global {
                    self.names.globals.insert(name.clone());
                }
                if container.exported {
                    self.names.properties.insert(name.clone());
                }
            }
        }
    }

    fn with_export_scope(&mut self, visit: impl FnOnce(&mut Self)) {
        self.export_depth += 1;
        visit(self);
        self.export_depth -= 1;
    }
}

impl<'ast> Visit<'ast> for AtKeepNameCollector<'ast> {
    fn visit_class(&mut self, class: &Class<'ast>) {
        let name = class
            .id
            .as_ref()
            .map(|identifier| identifier.name.to_string());
        if self.is_tagged(class.span.start) {
            self.insert_declaration_name(name.as_deref());
            for member in &class.body.body {
                self.insert_class_member(member);
            }
        }
        self.containers.push(AtKeepContainer {
            name,
            global: self.declaration_depth == 0 || self.export_depth > 0,
            exported: self.export_depth > 0,
        });
        self.declaration_depth += 1;
        walk::walk_class(self, class);
        self.declaration_depth -= 1;
        self.containers.pop();
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'ast>) {
        if self.is_tagged(method.span.start) {
            self.insert_key(&method.key);
            self.insert_current_container_name();
        }
        walk::walk_method_definition(self, method);
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'ast>) {
        if self.is_tagged(property.span.start) {
            self.insert_key(&property.key);
            self.insert_current_container_name();
        }
        walk::walk_property_definition(self, property);
    }

    fn visit_accessor_property(&mut self, property: &AccessorProperty<'ast>) {
        if self.is_tagged(property.span.start) {
            self.insert_key(&property.key);
            self.insert_current_container_name();
        }
        walk::walk_accessor_property(self, property);
    }

    fn visit_function(&mut self, function: &Function<'ast>, flags: oxc_syntax::scope::ScopeFlags) {
        if self.is_tagged(function.span.start) {
            self.insert_declaration_name(
                function
                    .id
                    .as_ref()
                    .map(|identifier| identifier.name.as_str()),
            );
        }
        self.declaration_depth += 1;
        walk::walk_function(self, function, flags);
        self.declaration_depth -= 1;
    }

    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'ast>) {
        if self.is_tagged(declaration.span.start) {
            for declarator in &declaration.declarations {
                for identifier in declarator.id.get_binding_identifiers() {
                    self.insert_declaration_name(Some(identifier.name.as_str()));
                }
            }
        }
        walk::walk_variable_declaration(self, declaration);
    }

    fn visit_ts_interface_declaration(&mut self, interface: &TSInterfaceDeclaration<'ast>) {
        let tagged = self.is_tagged(interface.span.start);
        for member in &interface.body.body {
            if tagged || self.is_tagged(member.span().start) {
                self.insert_declaration_name(Some(interface.id.name.as_str()));
                self.insert_interface_member(member);
            }
        }
        self.declaration_depth += 1;
        walk::walk_ts_interface_declaration(self, interface);
        self.declaration_depth -= 1;
    }

    fn visit_ts_enum_declaration(&mut self, declaration: &TSEnumDeclaration<'ast>) {
        let tagged = self.is_tagged(declaration.span.start);
        for member in &declaration.body.members {
            if tagged || self.is_tagged(member.span.start) {
                self.insert_declaration_name(Some(declaration.id.name.as_str()));
                match &member.id {
                    oxc_ast::ast::TSEnumMemberName::Identifier(identifier) => {
                        self.names.properties.insert(identifier.name.to_string());
                    }
                    oxc_ast::ast::TSEnumMemberName::String(literal)
                    | oxc_ast::ast::TSEnumMemberName::ComputedString(literal) => {
                        self.names.properties.insert(literal.value.to_string());
                    }
                    oxc_ast::ast::TSEnumMemberName::ComputedTemplateString(_) => {}
                }
            }
        }
        self.declaration_depth += 1;
        walk::walk_ts_enum_declaration(self, declaration);
        self.declaration_depth -= 1;
    }

    fn visit_export_declaration(&mut self, declaration: &ExportDeclaration<'ast>) {
        self.with_export_scope(|collector| walk::walk_export_declaration(collector, declaration));
    }

    fn visit_export_default_declaration(&mut self, declaration: &ExportDefaultDeclaration<'ast>) {
        self.with_export_scope(|collector| {
            walk::walk_export_default_declaration(collector, declaration)
        });
    }

    fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'ast>) {
        self.with_export_scope(|collector| {
            walk::walk_export_named_declaration(collector, declaration)
        });
    }
}

struct ReservedNameAstCollector {
    scan: ReservedNameSourceScan,
    source_kind: ObfuscationSourceKind,
    collect_all_names: bool,
    export_depth: usize,
    decorator_depth: usize,
    keep_string_properties: bool,
    keep_object_properties: bool,
    collect_exports: bool,
    collect_enum_members: bool,
    strip_sdk_argument_names: bool,
    sdk_argument_depth: usize,
}

impl ReservedNameAstCollector {
    fn new(
        source_kind: ObfuscationSourceKind,
        collect_all_names: bool,
        keep_string_properties: bool,
        keep_object_properties: bool,
        collect_exports: bool,
        collect_enum_members: bool,
        strip_sdk_argument_names: bool,
    ) -> Self {
        Self {
            scan: ReservedNameSourceScan::default(),
            source_kind,
            collect_all_names,
            export_depth: 0,
            decorator_depth: 0,
            keep_string_properties,
            keep_object_properties,
            collect_exports,
            collect_enum_members,
            strip_sdk_argument_names,
            sdk_argument_depth: 0,
        }
    }

    fn insert_property_name(&mut self, name: &str) {
        self.scan.names.insert(name.to_string());
        if matches!(
            self.source_kind,
            ObfuscationSourceKind::SdkApi | ObfuscationSourceKind::SdkComponent
        ) {
            self.scan.sdk_properties.insert(name.to_string());
            self.scan.sdk_locals.insert(name.to_string());
        }
        if self.source_kind == ObfuscationSourceKind::Project
            && (self.collect_all_names || self.export_depth > 0)
        {
            self.scan.export_properties.insert(name.to_string());
        }
    }

    fn strips_current_sdk_argument(&self) -> bool {
        self.strip_sdk_argument_names && self.sdk_argument_depth > 0 && self.is_sdk_source()
    }

    fn is_sdk_source(&self) -> bool {
        matches!(
            self.source_kind,
            ObfuscationSourceKind::SdkApi | ObfuscationSourceKind::SdkComponent
        )
    }

    fn insert_export_name(&mut self, name: &str) {
        self.insert_property_name(name);
        if self.source_kind == ObfuscationSourceKind::SdkApi {
            self.scan.sdk_globals.insert(name.to_string());
        } else if self.source_kind == ObfuscationSourceKind::Project {
            self.scan.export_globals.insert(name.to_string());
        }
    }

    fn insert_module_export_name(&mut self, name: &ModuleExportName<'_>) {
        match name {
            ModuleExportName::IdentifierName(identifier) => {
                self.insert_export_name(identifier.name.as_str());
            }
            ModuleExportName::IdentifierReference(identifier) => {
                self.insert_export_name(identifier.name.as_str());
            }
            ModuleExportName::StringLiteral(literal) => {
                self.insert_export_name(literal.value.as_str());
            }
        }
    }

    fn property_key_name<'key>(key: &'key PropertyKey<'_>) -> Option<&'key str> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str()),
            PropertyKey::StringLiteral(literal) => Some(literal.value.as_str()),
            PropertyKey::PrivateIdentifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn insert_property_key(&mut self, key: &PropertyKey<'_>) {
        if let Some(name) = Self::property_key_name(key) {
            self.insert_property_name(name);
        }
    }

    fn insert_struct_key(&mut self, key: &PropertyKey<'_>) {
        if let Some(name) = Self::property_key_name(key) {
            self.insert_property_name(name);
            self.scan.structs.insert(name.to_string());
        }
    }

    fn with_export_scope(&mut self, visit: impl FnOnce(&mut Self)) {
        self.export_depth +=
            usize::from(self.collect_exports || self.source_kind == ObfuscationSourceKind::SdkApi);
        visit(self);
        self.export_depth -=
            usize::from(self.collect_exports || self.source_kind == ObfuscationSourceKind::SdkApi);
    }
}

impl<'a> Visit<'a> for ReservedNameAstCollector {
    fn visit_identifier_name(&mut self, identifier: &IdentifierName<'a>) {
        if self.collect_all_names || self.decorator_depth > 0 {
            self.insert_property_name(identifier.name.as_str());
        }
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if self.collect_all_names || self.decorator_depth > 0 {
            self.insert_property_name(identifier.name.as_str());
        }
    }

    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        if self.sdk_argument_depth > 0 {
            if !self.strips_current_sdk_argument()
                && (self.collect_all_names || self.is_sdk_source())
            {
                self.insert_property_name(identifier.name.as_str());
            }
            return;
        }
        if self.export_depth > 0 {
            self.insert_export_name(identifier.name.as_str());
        } else if self.collect_all_names || self.decorator_depth > 0 {
            self.insert_property_name(identifier.name.as_str());
        }
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if self.export_depth > 0 {
            if let Some(identifier) = &function.id {
                self.insert_export_name(identifier.name.as_str());
            }
            let export_depth = std::mem::take(&mut self.export_depth);
            walk::walk_function(self, function, flags);
            self.export_depth = export_depth;
        } else {
            walk::walk_function(self, function, flags);
        }
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        let export_depth = std::mem::take(&mut self.export_depth);
        walk::walk_arrow_function_expression(self, function);
        self.export_depth = export_depth;
    }

    fn visit_property_key(&mut self, key: &PropertyKey<'a>) {
        let is_quoted = matches!(key, PropertyKey::StringLiteral(_));
        if self.collect_all_names
            || self.source_kind == ObfuscationSourceKind::SdkApi
            || self.export_depth > 0
            || (self.keep_string_properties && is_quoted)
        {
            self.insert_property_key(key);
            if let Some(name) = Self::property_key_name(key) {
                if self.keep_string_properties && is_quoted {
                    self.scan.strings.insert(name.to_string());
                }
            }
        }
        walk::walk_property_key(self, key);
    }

    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        if self.collect_all_names || self.decorator_depth > 0 {
            self.insert_property_name(literal.value.as_str());
        }
    }

    fn visit_ts_index_signature_name(&mut self, parameter: &TSIndexSignatureName<'a>) {
        if self.collect_all_names {
            self.insert_property_name(parameter.name.as_str());
        }
        walk::walk_ts_index_signature_name(self, parameter);
    }

    fn visit_formal_parameter(&mut self, parameter: &FormalParameter<'a>) {
        self.sdk_argument_depth += 1;
        walk::walk_formal_parameter(self, parameter);
        self.sdk_argument_depth -= 1;
    }

    fn visit_formal_parameter_rest(&mut self, parameter: &FormalParameterRest<'a>) {
        self.sdk_argument_depth += 1;
        walk::walk_formal_parameter_rest(self, parameter);
        self.sdk_argument_depth -= 1;
    }

    fn visit_ts_type_parameter(&mut self, parameter: &TSTypeParameter<'a>) {
        self.sdk_argument_depth += 1;
        walk::walk_ts_type_parameter(self, parameter);
        self.sdk_argument_depth -= 1;
    }

    fn visit_export_declaration(&mut self, declaration: &ExportDeclaration<'a>) {
        self.with_export_scope(|collector| walk::walk_export_declaration(collector, declaration));
    }

    fn visit_export_default_declaration(&mut self, declaration: &ExportDefaultDeclaration<'a>) {
        self.with_export_scope(|collector| {
            walk::walk_export_default_declaration(collector, declaration)
        });
    }

    fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'a>) {
        let collect = self.collect_exports || self.source_kind == ObfuscationSourceKind::SdkApi;
        if collect {
            self.export_depth += 1;
            for specifier in &declaration.specifiers {
                self.insert_module_export_name(&specifier.local);
                self.insert_module_export_name(&specifier.exported);
            }
            walk::walk_export_named_declaration(self, declaration);
            self.export_depth -= 1;
        } else {
            walk::walk_export_named_declaration(self, declaration);
        }
    }

    fn visit_export_specifier(&mut self, specifier: &ExportSpecifier<'a>) {
        if self.collect_exports || self.source_kind == ObfuscationSourceKind::SdkApi {
            self.insert_module_export_name(&specifier.local);
            self.insert_module_export_name(&specifier.exported);
        }
    }

    fn visit_computed_member_expression(&mut self, expression: &ComputedMemberExpression<'a>) {
        if self.keep_string_properties {
            if let Expression::StringLiteral(literal) = &expression.expression {
                self.insert_property_name(literal.value.as_str());
                self.scan.strings.insert(literal.value.to_string());
            }
        }
        walk::walk_computed_member_expression(self, expression);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if self.keep_object_properties {
            if let Some(name) = Self::property_key_name(&property.key) {
                self.insert_property_name(name);
                self.scan.objects.insert(name.to_string());
            }
        }
        walk::walk_object_property(self, property);
    }

    fn visit_decorator(&mut self, decorator: &Decorator<'a>) {
        self.decorator_depth += 1;
        walk::walk_decorator(self, decorator);
        self.decorator_depth -= 1;
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        if !class.decorators.is_empty() {
            if let Some(identifier) = class.id.as_ref() {
                self.insert_property_name(identifier.name.as_str());
            }
        }
        let is_view = class.heritage.as_ref().is_some_and(|heritage| {
            matches!(
                &heritage.expression,
                Expression::Identifier(identifier)
                    if matches!(identifier.name.as_str(), "ViewPU" | "ViewV2" | "PUV2ViewBase")
            )
        });
        if is_view {
            for member in &class.body.body {
                match member {
                    ClassElement::MethodDefinition(member) => self.insert_struct_key(&member.key),
                    ClassElement::PropertyDefinition(member) => self.insert_struct_key(&member.key),
                    ClassElement::AccessorProperty(member) => self.insert_struct_key(&member.key),
                    ClassElement::ETSOverloadDeclaration(member) => {
                        self.insert_struct_key(&member.key)
                    }
                    ClassElement::StaticBlock(_)
                    | ClassElement::TSIndexSignature(_)
                    | ClassElement::TSCallSignatureDeclaration(_) => {}
                }
            }
        }
        walk::walk_class(self, class);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        if !method.decorators.is_empty() {
            self.insert_property_key(&method.key);
        }
        walk::walk_method_definition(self, method);
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'a>) {
        if !property.decorators.is_empty() {
            self.insert_property_key(&property.key);
        }
        walk::walk_property_definition(self, property);
    }

    fn visit_accessor_property(&mut self, property: &AccessorProperty<'a>) {
        if !property.decorators.is_empty() {
            self.insert_property_key(&property.key);
        }
        walk::walk_accessor_property(self, property);
    }

    fn visit_struct_statement(&mut self, statement: &StructStatement<'a>) {
        for member in &statement.body.body {
            match member {
                StructElement::MethodDefinition(member) => self.insert_struct_key(&member.key),
                StructElement::PropertyDefinition(member) => self.insert_struct_key(&member.key),
                StructElement::AccessorProperty(member) => self.insert_struct_key(&member.key),
                StructElement::ETSOverloadDeclaration(member) => {
                    self.insert_struct_key(&member.key)
                }
                StructElement::StaticBlock(_) | StructElement::TSIndexSignature(_) => {}
            }
        }
        walk::walk_struct_statement(self, statement);
    }

    fn visit_ts_enum_declaration(&mut self, declaration: &TSEnumDeclaration<'a>) {
        let contains_initializer_identifier = declaration.body.members.iter().any(|member| {
            member
                .initializer
                .as_ref()
                .is_some_and(|initializer| EnumInitializerIdentifierDetector::contains(initializer))
        });
        if self.collect_enum_members || contains_initializer_identifier {
            for member in &declaration.body.members {
                let name = match &member.id {
                    TSEnumMemberName::Identifier(identifier) => Some(identifier.name.as_str()),
                    TSEnumMemberName::String(literal)
                    | TSEnumMemberName::ComputedString(literal) => Some(literal.value.as_str()),
                    TSEnumMemberName::ComputedTemplateString(_) => None,
                };
                if let Some(name) = name {
                    self.insert_property_name(name);
                    self.scan.enums.insert(name.to_string());
                }
            }
        }
        walk::walk_ts_enum_declaration(self, declaration);
    }
}

struct EnumInitializerIdentifierDetector {
    found: bool,
}

impl EnumInitializerIdentifierDetector {
    fn contains(expression: &Expression<'_>) -> bool {
        let mut detector = Self { found: false };
        detector.visit_expression(expression);
        detector.found
    }
}

impl<'a> Visit<'a> for EnumInitializerIdentifierDetector {
    fn visit_identifier_reference(&mut self, _identifier: &IdentifierReference<'a>) {
        self.found = true;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeObfuscationConfig {
    abc_file_path: String,
    obf_abc_file_path: String,
    obf_pa_file_path: String,
    compile_sdk_version: String,
    target_api_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_api_sub_version: Option<String>,
    files_info_path: String,
    entry_package_info: String,
    source_maps_path: String,
    default_name_cache_path: String,
    skipped_remote_har_list: Vec<String>,
    #[serde(rename = "useNormalizedOHMUrl")]
    use_normalized_ohm_url: bool,
    obfuscation_rules: BytecodeObfuscationRules,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeObfuscationRules {
    disable_obfuscation: bool,
    compact: bool,
    enable_export_obfuscation: bool,
    enable_remove_log: bool,
    enable_decorator_obfuscation: bool,
    print_name_cache: String,
    apply_name_cache: String,
    reserved_names: Vec<String>,
    record_name_white_list: Vec<String>,
    property_obfuscation: BytecodePropertyObfuscation,
    toplevel_obfuscation: BytecodeToplevelObfuscation,
    file_name_obfuscation: BytecodeFileNameObfuscation,
    keep_options: BytecodeKeepOptions,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodePropertyObfuscation {
    enable: bool,
    reserved_properties: Vec<String>,
    universal_reserved_properties: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeToplevelObfuscation {
    enable: bool,
    reserved_toplevel_names: Vec<String>,
    universal_reserved_toplevel_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeFileNameObfuscation {
    enable: bool,
    reserved_file_names: Vec<String>,
    universal_reserved_file_names: Vec<String>,
    reserved_remote_har_pkg_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeKeepOptions {
    enable: bool,
    keep_paths: Vec<String>,
}

struct BytecodeObfuscationConfigBuilder<'a> {
    options: &'a ObfuscationOptions,
    rules: &'a ObfuscationRules,
    adapter: &'a ArkguardAdapterOptions,
    input: &'a BytecodeObfuscationInput,
    paths: &'a BytecodeObfuscationPaths,
    inventory: ArkguardReservedNameInventory,
}

impl<'a> BytecodeObfuscationConfigBuilder<'a> {
    fn new(
        options: &'a ObfuscationOptions,
        rules: &'a ObfuscationRules,
        adapter: &'a ArkguardAdapterOptions,
        input: &'a BytecodeObfuscationInput,
        paths: &'a BytecodeObfuscationPaths,
        inventory: ArkguardReservedNameInventory,
    ) -> Self {
        Self {
            options,
            rules,
            adapter,
            input,
            paths,
            inventory,
        }
    }

    fn build(self) -> Result<BytecodeObfuscationConfig, ObfuscationError> {
        let (reserved_properties, universal_reserved_properties) =
            Self::partition_reserved_names(&self.rules.keep_property_names);
        let (reserved_toplevel_names, universal_reserved_toplevel_names) =
            Self::partition_reserved_names(&self.rules.keep_global_names);
        let (reserved_file_names, universal_reserved_file_names) =
            Self::partition_reserved_names(&self.rules.keep_file_names);
        let reserved_file_names = reserved_file_names
            .into_iter()
            .map(|name| {
                if !name.starts_with('.') && name.bytes().filter(|byte| *byte == b'.').count() == 1
                {
                    name.rsplit_once('.')
                        .map(|(stem, _)| stem.to_string())
                        .unwrap_or(name)
                } else {
                    name
                }
            })
            .collect();
        let mut property_reserved = reserved_properties.into_iter().collect::<BTreeSet<_>>();
        property_reserved.extend(BYTECODE_PROPERTY_WHITE_LIST.into_iter().map(str::to_string));
        let keep_paths = self.keep_paths()?;
        let output_paths = RuleOutputPathResolver::new(self.options);
        let print_name_cache = output_paths.serialized(self.rules.print_name_cache.as_deref())?;
        let apply_name_cache = output_paths.serialized(self.rules.apply_name_cache.as_deref())?;
        let mut record_name_white_list = self.input.record_name_white_list.clone();
        record_name_white_list.sort();
        record_name_white_list.dedup();
        let skipped_remote_har_list = self
            .adapter
            .byte_code_har_info
            .as_object()
            .map(|entries| entries.keys().cloned().collect())
            .unwrap_or_default();
        Ok(BytecodeObfuscationConfig {
            abc_file_path: ArtifactPathSerializer::path_to_slash_string(&self.paths.origin_abc)?,
            obf_abc_file_path: ArtifactPathSerializer::path_to_slash_string(
                &self.paths.obfuscated_abc,
            )?,
            obf_pa_file_path: self
                .paths
                .obfuscated_pa
                .as_deref()
                .map(ArtifactPathSerializer::path_to_slash_string)
                .transpose()?
                .unwrap_or_default(),
            compile_sdk_version: self.adapter.compile_sdk_version.clone(),
            target_api_version: self.adapter.compatible_sdk_version,
            target_api_sub_version: self.adapter.compatible_sdk_version_stage.clone(),
            files_info_path: ArtifactPathSerializer::path_to_slash_string(
                &self.adapter.cache_path.join("filesInfo.txt"),
            )?,
            entry_package_info: self.adapter.entry_package_info.clone(),
            source_maps_path: ArtifactPathSerializer::path_to_slash_string(
                &self.paths.source_maps,
            )?,
            default_name_cache_path: ArtifactPathSerializer::path_to_slash_string(
                &self.paths.default_name_cache,
            )?,
            skipped_remote_har_list,
            use_normalized_ohm_url: self.adapter.use_normalized,
            obfuscation_rules: BytecodeObfuscationRules {
                disable_obfuscation: false,
                compact: self.rules.compact,
                enable_export_obfuscation: self.rules.enable_export_obfuscation,
                enable_remove_log: self.rules.remove_log,
                enable_decorator_obfuscation: false,
                print_name_cache,
                apply_name_cache,
                reserved_names: self.inventory.names.into_iter().collect(),
                record_name_white_list,
                property_obfuscation: BytecodePropertyObfuscation {
                    enable: self.rules.enable_property_obfuscation,
                    reserved_properties: property_reserved.into_iter().collect(),
                    universal_reserved_properties,
                },
                toplevel_obfuscation: BytecodeToplevelObfuscation {
                    enable: self.rules.enable_toplevel_obfuscation,
                    reserved_toplevel_names,
                    universal_reserved_toplevel_names,
                },
                file_name_obfuscation: BytecodeFileNameObfuscation {
                    enable: self.rules.enable_file_name_obfuscation,
                    reserved_file_names,
                    universal_reserved_file_names,
                    reserved_remote_har_pkg_names: Vec::new(),
                },
                keep_options: BytecodeKeepOptions {
                    // `BytecodeObfuscationConfig` freezes this flag before
                    // collectSourcesWhiteList later injects ResourceTable.ts.
                    enable: !self.rules.keep_source_paths.is_empty(),
                    keep_paths,
                },
            },
        })
    }

    fn partition_reserved_names(
        names: &rustc_hash::FxHashSet<String>,
    ) -> (Vec<String>, Vec<String>) {
        let mut specific = Vec::new();
        let mut universal = Vec::new();
        for name in names {
            if name.contains(['*', '?']) {
                universal.push(crate::arkguard::ArkguardWildcard::regex_string(name));
            } else {
                specific.push(name.clone());
            }
        }
        specific.sort();
        universal.sort();
        (specific, universal)
    }

    fn keep_paths(&self) -> Result<Vec<String>, ObfuscationError> {
        let mut records = BTreeMap::new();
        for record in &self.input.module_records {
            records.insert(
                ArtifactPathSerializer::path_to_slash_string(&record.source_path)?,
                record.record_name.clone(),
            );
        }
        let mut keep_paths = BTreeSet::new();
        for (source_path, record_name) in &records {
            let excluded = self.rules.exclude_source_paths.iter().any(|configured| {
                configured == source_path
                    || crate::arkguard::ArkguardWildcard::new(configured, true).matches(source_path)
            });
            if excluded {
                continue;
            }
            let included = self.rules.keep_source_paths.iter().any(|configured| {
                configured == source_path
                    || crate::arkguard::ArkguardWildcard::new(configured, true).matches(source_path)
                    || (Path::new(configured).is_dir()
                        && Path::new(source_path).starts_with(configured))
            });
            if included {
                keep_paths.insert(record_name.clone());
            }
        }
        if let Some(resource_table) = self.adapter.resource_table_path.as_ref() {
            let resource_table = ArtifactPathSerializer::path_to_slash_string(resource_table)?;
            keep_paths.insert(
                records
                    .get(&resource_table)
                    .cloned()
                    .unwrap_or(resource_table),
            );
        }
        Ok(keep_paths.into_iter().collect())
    }
}

struct NameCacheProcessor;

impl NameCacheProcessor {
    fn source_file_mapping(name_cache: &Map<String, Value>) -> BTreeMap<String, String> {
        name_cache
            .iter()
            .filter(|(key, _)| !NAME_CACHE_METADATA_KEYS.contains(&key.as_str()))
            .filter_map(|(_, value)| {
                let value = value.as_object()?;
                Some((
                    value.get("OriSourceFile")?.as_str()?.to_string(),
                    value.get("ObfSourceFile")?.as_str()?.to_string(),
                ))
            })
            .collect()
    }

    fn rename_source_map_keys(
        source_maps: Map<String, Value>,
        mapping: &BTreeMap<String, String>,
    ) -> Map<String, Value> {
        source_maps
            .into_iter()
            .map(|(key, value)| (mapping.get(&key).cloned().unwrap_or(key), value))
            .collect()
    }

    fn remap_line_ranges(
        name_cache: Map<String, Value>,
        source_maps: &Map<String, Value>,
    ) -> Result<Map<String, Value>, ObfuscationError> {
        let mut updated = Map::new();
        for (key, mut value) in name_cache {
            let Some(entry) = value.as_object_mut() else {
                updated.insert(key, value);
                continue;
            };
            let Some(source_key) = entry
                .get("OriSourceFile")
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                updated.insert(key, value);
                continue;
            };
            let Some(source_map_value) = source_maps.get(&source_key) else {
                // Latest `processNameCacheEntry` omits file entries which no
                // longer have a corresponding source-map record.
                continue;
            };
            let source_map = OwnedSourceMap::from_json_string(&source_map_value.to_string())
                .map_err(
                    |error| ObfuscationError::BytecodeObfuscationSourceMapInvalid {
                        source_file: source_key,
                        message: error.to_string(),
                    },
                )?;
            let lookup = source_map.generate_lookup_table();
            for cache_key in ["IdentifierCache", "MemberMethodCache"] {
                let Some(items) = entry.get_mut(cache_key).and_then(Value::as_object_mut) else {
                    continue;
                };
                let old_items = std::mem::take(items);
                for (range, obfuscated_name) in old_items {
                    let remapped = (|| {
                        // SDK `processItemKey` consumes the first three
                        // colon-delimited fields and ignores any remainder.
                        let mut parts = range.split(':');
                        let scope = parts.next()?;
                        let start = parts.next()?.parse::<u32>().ok()?;
                        let end = parts.next()?.parse::<u32>().ok()?;
                        let start = source_map
                            .lookup_source_view_token_approx(&lookup, start.saturating_sub(1), 0)?
                            .get_src_line()
                            + 1;
                        let end = source_map
                            .lookup_source_view_token_approx(&lookup, end.saturating_sub(1), 0)?
                            .get_src_line()
                            + 1;
                        Some(format!("{scope}:{start}:{end}"))
                    })()
                    .unwrap_or(range);
                    items.insert(remapped, obfuscated_name);
                }
            }
            updated.insert(key, value);
        }
        Ok(updated)
    }
}
