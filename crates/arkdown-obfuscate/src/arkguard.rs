use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_ets_transform::HvigorTypescriptPrinter;
use arkdown_path::{
    canonicalize_or_owned, ArtifactPathSerializer, ModuleSpecifierText, RelPath, SpecifierKind,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AccessorProperty, BindingIdentifier, ChainElement, Class, ComputedMemberExpression,
    ExportAllDeclaration, ExportDeclaration, ExportDefaultDeclaration, ExportFromDeclaration,
    ExportNamedDeclaration, Expression, FormalParameter, FormalParameterRest, Function,
    IdentifierName, IdentifierReference, ImportDeclaration, ImportExpression,
    LazyImportDeclaration, MethodDefinition, MethodDefinitionKind, ModuleExportName,
    ObjectProperty, PrivateFieldExpression, Program, PropertyDefinition, PropertyKey, Statement,
    StaticMemberExpression, StringLiteral, TSEnumDeclaration, TSEnumMemberName,
    TSInterfaceDeclaration, TSMethodSignature, TSPropertySignature, VariableDeclaration,
};
use oxc_ast_visit::{walk, walk_mut, Visit, VisitMut};
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions, IndentChar};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_str::{Ident, Str};
use oxc_syntax::reference::ReferenceId;
use oxc_syntax::symbol::{SymbolFlags, SymbolId};
use rolldown_sourcemap::{OwnedSourceMap, SourceMapBuilder};
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::{Map, Value};

use arkdown_core::HvigorModuleSourceFile;

use crate::{ObfuscationError, ObfuscationOptions, ObfuscationRules};

impl ObfuscationRules {
    pub(crate) fn resolve_keep_dts_names(&mut self) -> Result<(), ObfuscationError> {
        let names = crate::bytecode::ArkguardReservedNameInventory::collect_keep_dts_names(
            &self.keep_dts_paths,
            self,
        )?;
        self.keep_global_names.extend(names.iter().cloned());
        self.keep_property_names.extend(names);
        Ok(())
    }
}

/// SDK and build metadata required by native source obfuscation and PandaGuard.
#[derive(Debug, Clone)]
pub struct ArkguardAdapterOptions {
    /// Native PandaGuard executable resolved alongside SDK `es2abc`.
    pub panda_guard_path: PathBuf,
    /// ArkGuard package root. PandaGuard consumes its reserved-name presets.
    pub arkguard_package: PathBuf,
    pub module_root: PathBuf,
    pub project_root: PathBuf,
    pub project_path: PathBuf,
    pub cache_path: PathBuf,
    pub ace_module_build: PathBuf,
    pub compile_sdk_version: String,
    pub compatible_sdk_version: u32,
    pub compatible_sdk_version_stage: Option<String>,
    pub entry_package_info: String,
    pub resource_table_path: Option<PathBuf>,
    pub use_normalized: bool,
    pub byte_code_har_info: serde_json::Value,
    /// Path-like values passed by hvigor to
    /// `ConfigResolver.collectResevedFileNameInIDEConfig`.
    pub reserved_file_paths: Vec<String>,
    /// Properties from `obfuscateWhiteList.json5::ReservedPropertyNames`.
    pub arkui_reserved_property_names: Vec<String>,
    /// Additional properties selected only by `strip-system-api-args`.
    pub arkui_optimized_reserved_property_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ObfuscationModuleFile {
    pub build_path: PathBuf,
    pub original_path: PathBuf,
    pub relative_path: String,
    /// Derived ABC record. Explicit `meta.ohmurl` records are omitted because
    /// ModuleMode preserves them while remapping filename-derived records.
    pub record_name: Option<String>,
    /// Latest ets-loader never renames files physically owned by `oh_modules`.
    pub rename_file: bool,
    /// Executable-source cache key reused while transforming its declaration.
    pub source_name_cache_key: Option<String>,
    pub source_map_key: String,
    pub previous_source_map: Option<serde_json::Value>,
    /// Post-ArkUI CompileArkTS source frozen by the build plan.
    pub source: Arc<str>,
    pub insight_intent_global_names: Vec<String>,
    pub insight_intent_property_names: Vec<String>,
}

/// Immutable source inventory used to derive ArkGuard's module-wide reserved
/// names without reading mutable project or SDK files during execution.
#[derive(Debug, Clone)]
pub struct ObfuscationSource {
    pub path: PathBuf,
    pub source: Arc<str>,
    pub sdk_declaration: bool,
}

#[derive(Debug, Clone)]
pub struct ObfuscationModuleInput {
    pub files: Vec<ObfuscationModuleFile>,
    pub inventory_sources: Vec<ObfuscationSource>,
    /// Resolved Rolldown module edges keyed by original source path. ArkGuard
    /// uses the transitive dependencies of `-keep` files when preserving
    /// exported names during export obfuscation.
    pub module_dependencies: BTreeMap<PathBuf, Vec<PathBuf>>,
    /// Source-map entries not owned by the executable files in `files`, such
    /// as merged ByteCode HAR maps. ArkGuard retains them verbatim.
    pub preserved_source_maps: BTreeMap<String, Value>,
    pub source_maps_path: Option<PathBuf>,
    /// CompileArkTS cache and packaging mirrors written from the same final
    /// obfuscated source-map object as `source_maps_path`.
    pub source_map_mirror_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ObfuscationModuleOutput {
    pub name_cache_path: PathBuf,
    pub files: Vec<ObfuscationModuleFileOutput>,
}

/// Exact source-to-output mapping produced by one native ArkGuard run.
#[derive(Debug, Clone)]
pub struct ObfuscationModuleFileOutput {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub source_map_key: String,
    pub obfuscated_source_map_key: String,
    pub record_name: Option<String>,
    pub obfuscated_record_name: Option<String>,
}

impl ObfuscationModuleOutput {
    /// Apply filename-obfuscation metadata to the ModuleMode inputs consumed
    /// by es2abc. This mirrors `ModuleMode.addModuleInfoItem`, which derives
    /// physical paths, source-map keys, records, and imported records from the
    /// obfuscated filename in one step.
    pub fn apply_to_module_sources(
        &self,
        sources: &mut [HvigorModuleSourceFile],
    ) -> Result<(), ObfuscationError> {
        let files_by_input = self
            .files
            .iter()
            .map(|file| (&file.input_path, file))
            .collect::<BTreeMap<_, _>>();
        let record_names = self
            .files
            .iter()
            .filter_map(|file| {
                file.record_name
                    .as_ref()
                    .zip(file.obfuscated_record_name.as_ref())
            })
            .filter(|(original, obfuscated)| original != obfuscated)
            .collect::<BTreeMap<_, _>>();
        for source in sources.iter_mut() {
            let file = files_by_input.get(&source.path).ok_or_else(|| {
                ObfuscationError::ArkguardOutputMappingMissing(source.path.clone())
            })?;
            source.path.clone_from(&file.output_path);
            source
                .source_file
                .clone_from(&file.obfuscated_source_map_key);
            if let Some(record_name) = file.obfuscated_record_name.as_ref() {
                source.record_name.clone_from(record_name);
            }
        }
        for source in sources {
            for imported in &mut source.imported_record_names {
                if let Some(obfuscated) = record_names.get(imported) {
                    imported.clone_from(obfuscated);
                }
            }
        }
        Ok(())
    }
}

impl ObfuscationModuleFile {
    pub(crate) fn is_declaration(&self) -> bool {
        self.relative_path.ends_with(".d.ts") || self.relative_path.ends_with(".d.ets")
    }

    fn source_name_cache_key(&self) -> &str {
        self.source_name_cache_key
            .as_deref()
            .unwrap_or(&self.relative_path)
    }
}

pub(crate) struct ArkguardKeepPlan {
    source_paths: FxHashSet<PathBuf>,
    pub(crate) export_dependency_paths: FxHashSet<PathBuf>,
}

impl ObfuscationModuleInput {
    pub(crate) fn keep_plan(
        &self,
        rules: &ObfuscationRules,
        adapter: &ArkguardAdapterOptions,
    ) -> ArkguardKeepPlan {
        let resource_table = adapter
            .resource_table_path
            .as_deref()
            .map(canonicalize_or_owned);
        let source_paths = self
            .files
            .iter()
            .filter(|file| {
                rules.keeps_source(&file.original_path, &file.relative_path)
                    || resource_table
                        .as_ref()
                        .is_some_and(|path| path == &canonicalize_or_owned(&file.original_path))
            })
            .map(|file| file.original_path.clone())
            .collect::<FxHashSet<_>>();
        if !rules.enable_export_obfuscation || source_paths.is_empty() {
            return ArkguardKeepPlan {
                source_paths,
                export_dependency_paths: FxHashSet::default(),
            };
        }

        let project_sources = self
            .files
            .iter()
            .filter(|file| !file.is_declaration())
            .map(|file| file.original_path.clone())
            .collect::<FxHashSet<_>>();
        let mut export_dependency_paths = FxHashSet::default();
        let mut pending = source_paths.iter().cloned().collect::<Vec<_>>();
        while let Some(path) = pending.pop() {
            if !project_sources.contains(&path) || !export_dependency_paths.insert(path.clone()) {
                continue;
            }
            if let Some(dependencies) = self.module_dependencies.get(&path) {
                pending.extend(dependencies.iter().cloned());
            }
        }
        ArkguardKeepPlan {
            source_paths,
            export_dependency_paths,
        }
    }
}

/// Native owner for ArkGuard's module-scoped source-obfuscation lifecycle.
pub(crate) struct ArkguardModuleRunner<'a> {
    options: &'a ObfuscationOptions,
    rules: &'a ObfuscationRules,
    adapter: &'a ArkguardAdapterOptions,
    identifier_names: OrderedNameGenerator,
    file_names: OrderedNameGenerator,
    cache: ArkguardNameCache,
    reserved_names: FxHashSet<String>,
    reserved_properties: FxHashSet<String>,
    reserved_file_names: FxHashSet<String>,
    obfuscated_paths: BTreeMap<String, String>,
}

impl<'a> ArkguardModuleRunner<'a> {
    pub(crate) fn new(
        options: &'a ObfuscationOptions,
        rules: &'a ObfuscationRules,
        adapter: &'a ArkguardAdapterOptions,
    ) -> Result<Self, ObfuscationError> {
        let cache = ArkguardNameCache::load(options, adapter, rules)?;
        let mut reserved_names = rules.keep_global_names.clone();
        let mut reserved_properties = rules.keep_property_names.clone();
        let mut reserved_file_names = FxHashSet::default();
        for configured in &rules.keep_file_names {
            for segment in configured.split(['/', '\\']) {
                reserved_file_names.insert(segment.to_string());
                let (stem, extension) = ArkguardFileName::split(segment);
                reserved_file_names.insert(stem.to_string());
                reserved_file_names.insert(extension.to_string());
            }
        }
        reserved_names.extend(cache.all_mangled_names());
        reserved_properties.extend(cache.all_mangled_names());
        reserved_properties.extend(adapter.arkui_reserved_property_names.iter().cloned());
        if rules.strip_system_api_args {
            reserved_properties.extend(
                adapter
                    .arkui_optimized_reserved_property_names
                    .iter()
                    .cloned(),
            );
        }
        reserved_file_names.extend(cache.file_name_cache.values().cloned());
        reserved_file_names.extend(["".to_string(), ".".to_string(), "..".to_string()]);
        for configured in &adapter.reserved_file_paths {
            for segment in configured.split(['/', '\\']) {
                reserved_file_names.insert(segment.to_string());
                let (stem, extension) = ArkguardFileName::split(segment);
                reserved_file_names.insert(stem.to_string());
                reserved_file_names.insert(extension.to_string());
            }
        }
        for path in [
            &adapter.module_root,
            &adapter.project_root,
            &adapter.project_path,
            &adapter.cache_path,
            &adapter.ace_module_build,
        ] {
            reserved_file_names.extend(
                path.components()
                    .filter_map(|part| part.as_os_str().to_str().map(str::to_owned)),
            );
        }
        Ok(Self {
            options,
            rules,
            adapter,
            identifier_names: OrderedNameGenerator::default(),
            file_names: OrderedNameGenerator::default(),
            cache,
            reserved_names,
            reserved_properties,
            reserved_file_names,
            obfuscated_paths: BTreeMap::new(),
        })
    }

    pub(crate) fn run(
        mut self,
        mut input: ObfuscationModuleInput,
    ) -> Result<ObfuscationModuleOutput, ObfuscationError> {
        self.run_files(&mut input, true)
    }

    pub(crate) fn run_declarations(
        mut self,
        mut input: ObfuscationModuleInput,
    ) -> Result<ObfuscationModuleOutput, ObfuscationError> {
        input.files.retain(ObfuscationModuleFile::is_declaration);
        self.run_files(&mut input, false)
    }

    fn run_files(
        &mut self,
        input: &mut ObfuscationModuleInput,
        prune_deleted_files: bool,
    ) -> Result<ObfuscationModuleOutput, ObfuscationError> {
        fs_err::create_dir_all(&self.options.cache_dir)?;
        input
            .files
            .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let keep_plan = input.keep_plan(self.rules, self.adapter);
        let inventory = crate::bytecode::ArkguardReservedNameInventory::build_for_sources(
            self.adapter,
            self.rules,
            &input.inventory_sources,
            &input.files,
            &keep_plan.export_dependency_paths,
        )?;
        self.reserved_names.extend(inventory.names.iter().cloned());
        self.reserved_properties.extend(inventory.names);
        self.register_insight_intent_names(&input.files);
        self.plan_file_names(&input.files);
        let record_names = self.record_name_mappings(&input.files);
        let mut previous_outputs = self.previous_output_paths(&input.files);
        if prune_deleted_files {
            previous_outputs.extend(self.cache.prune_deleted_files(&input.files, self.adapter)?);
        }

        let mut source_maps = std::mem::take(&mut input.preserved_source_maps);
        let mut staged = Vec::with_capacity(input.files.len());
        for file in &input.files {
            let transformed = self.transform_file(file, &keep_plan.source_paths, &record_names)?;
            if let Some(source_map) = transformed.source_map.as_ref() {
                source_maps.insert(
                    transformed.obfuscated_source_map_key.clone(),
                    source_map.clone(),
                );
            }
            staged.push(transformed);
        }

        let mut files = Vec::with_capacity(staged.len());
        for output in staged {
            if let Some(parent) = output.output_path.parent() {
                fs_err::create_dir_all(parent)?;
            }
            let temporary = output.output_path.with_extension("arkdown-obfuscate-tmp");
            fs_err::write(&temporary, output.code.as_bytes())?;
            fs_err::rename(&temporary, &output.output_path)?;
            if output.input_path != output.output_path && output.input_path.is_file() {
                fs_err::remove_file(&output.input_path)?;
            }
            files.push(ObfuscationModuleFileOutput {
                input_path: output.input_path,
                output_path: output.output_path,
                source_map_key: output.source_map_key,
                obfuscated_source_map_key: output.obfuscated_source_map_key,
                record_name: output.record_name,
                obfuscated_record_name: output.obfuscated_record_name,
            });
        }
        let live_outputs = files
            .iter()
            .map(|file| file.output_path.clone())
            .collect::<BTreeSet<_>>();
        for stale in previous_outputs.difference(&live_outputs) {
            if stale.is_file() {
                fs_err::remove_file(stale)?;
            }
        }

        if let Some(source_maps_path) = input.source_maps_path.as_ref() {
            let bytes = serde_json::to_vec_pretty(&source_maps)?;
            let mut outputs = input
                .source_map_mirror_paths
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            outputs.insert(source_maps_path.clone());
            for output in outputs {
                if let Some(parent) = output.parent() {
                    fs_err::create_dir_all(parent)?;
                }
                fs_err::write(output, &bytes)?;
            }
        }
        let name_cache_path = self.cache.write(self.options, self.adapter, self.rules)?;
        Ok(ObfuscationModuleOutput {
            name_cache_path,
            files,
        })
    }

    fn register_insight_intent_names(&mut self, files: &[ObfuscationModuleFile]) {
        for file in files {
            self.reserved_names
                .extend(file.insight_intent_global_names.iter().cloned());
            self.reserved_properties
                .extend(file.insight_intent_property_names.iter().cloned());
        }
    }

    fn transform_file(
        &mut self,
        file: &ObfuscationModuleFile,
        keep_source_paths: &FxHashSet<PathBuf>,
        record_names: &BTreeMap<String, String>,
    ) -> Result<NativeSourceOutput, ObfuscationError> {
        let source = file.source.as_ref();
        let is_declaration = file.is_declaration();
        let keep_source = keep_source_paths.contains(&file.original_path);

        let allocator = Allocator::default();
        let source_type = SourceType::from_path(&file.build_path)
            .unwrap_or_else(|_| SourceType::ts().with_module(true));
        let parsed = Parser::new(&allocator, source, source_type).parse();
        if !parsed.diagnostics.is_empty() {
            return Err(ObfuscationError::ParseError {
                file: ArtifactPathSerializer::path_to_slash_string(&file.build_path)?,
                message: parsed
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            });
        }
        let mut program = parsed.program;
        let kept_comment_targets = if is_declaration && self.rules.remove_comments {
            DeclarationCommentTargetCollector::new(self.rules).collect(&program)
        } else {
            BTreeSet::new()
        };
        if is_declaration && self.rules.remove_comments {
            program.comments.retain(|comment| {
                DeclarationCommentTargetCollector::keeps_attached_comment(
                    source,
                    &kept_comment_targets,
                    comment.attached_to,
                )
            });
        }
        if self.rules.remove_log || !self.rules.remove_no_side_effect_calls.is_empty() {
            ExpressionStatementRemover::new(source, self.rules).transform(&mut program);
        }
        let semantic_return = SemanticBuilder::new()
            .with_build_nodes(true)
            .build(&program);
        if !semantic_return.diagnostics.is_empty() {
            return Err(ObfuscationError::ParseError {
                file: ArtifactPathSerializer::path_to_slash_string(&file.build_path)?,
                message: semantic_return
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            });
        }
        let semantic = semantic_return.semantic;
        let policy = IdentifierPolicy::collect(&program);
        let (symbol_plan, identifier_cache) = if keep_source {
            (Vec::new(), BTreeMap::new())
        } else {
            self.plan_symbol_names(file, &semantic, &policy)
        };
        drop(semantic);

        if self.rules.keep_object_props {
            let kept = ObjectPropertyKeepCollector::collect(&program);
            self.reserved_properties.extend(kept);
        }
        let mut property_collector = PropertyNameCollector::new(
            self.rules.enable_string_property_obfuscation,
            self.rules.enable_export_obfuscation,
        );
        property_collector.visit_program(&program);
        let property_map = if keep_source {
            FxHashMap::default()
        } else {
            self.plan_property_names(property_collector.names)
        };
        let member_method_cache =
            MemberMethodCacheCollector::new(source, &property_map, &symbol_plan).collect(&program);
        let member_method_cache = self.remap_member_method_cache(file, member_method_cache)?;
        NativeArkguardAstTransform::new(
            &allocator,
            &property_map,
            &self.cache.file_name_cache,
            record_names,
            file.rename_file,
            self.rules.enable_string_property_obfuscation,
            self.rules.enable_export_obfuscation,
        )
        .transform(&mut program);
        let semantic_return = SemanticBuilder::new()
            .with_build_nodes(true)
            .build(&program);
        if !semantic_return.diagnostics.is_empty() {
            return Err(ObfuscationError::ParseError {
                file: ArtifactPathSerializer::path_to_slash_string(&file.build_path)?,
                message: semantic_return
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            });
        }
        let mut scoping = semantic_return.semantic.into_scoping();
        let mut synthetic_symbol_spans = self.apply_symbol_names(&mut scoping, &symbol_plan);
        synthetic_symbol_spans.transform(&mut program);

        let obfuscated_relative_path = self.obfuscated_relative_path(&file.relative_path);
        let logical_path =
            ArkguardLogicalFilePath::new(&file.relative_path, &obfuscated_relative_path);
        let output_path = self.output_path(file, &obfuscated_relative_path);
        let compact = self.rules.compacts(&file.original_path);
        let mut codegen_options = if compact {
            CodegenOptions::minify()
        } else {
            CodegenOptions {
                // `ArkObfuscator.createObfsPrinter` delegates to the
                // TypeScript printer, whose default indentation is four
                // spaces. OXC defaults to one tab.
                indent_char: IndentChar::Space,
                indent_width: 4,
                ..CodegenOptions::default()
            }
        };
        // `Initializer.initArkGuardConfig` always sets mRemoveComments for
        // executable sources. `-remove-comments` controls declaration files.
        if !is_declaration {
            codegen_options.comments = CommentOptions::disabled();
        }
        if !is_declaration {
            codegen_options.source_map_path = Some(PathBuf::from(&file.relative_path));
        }
        let (code, generated_map) = if compact {
            let generated = Codegen::new()
                .with_options(codegen_options)
                .with_scoping(Some(scoping))
                .build(&program);
            (
                generated.code,
                generated.map.map(oxc_sourcemap::SourceMap::into_owned),
            )
        } else if is_declaration {
            (
                HvigorTypescriptPrinter::print_with_scoping(&program, codegen_options, scoping),
                None,
            )
        } else {
            let printed = HvigorTypescriptPrinter::print_with_source_map_and_scoping(
                &program,
                codegen_options,
                source,
                scoping,
            );
            let map = printed
                .source_map
                .as_deref()
                .map(|source_map| {
                    OwnedSourceMap::from_json_string(source_map)
                        .map(|map| map.as_source_map().clone().into_owned())
                })
                .transpose()
                .map_err(|error| ObfuscationError::CodegenError(error.to_string()))?;
            (printed.code, map)
        };
        let source_map = if is_declaration {
            None
        } else {
            generated_map
                .as_ref()
                .map(|map| self.compose_source_map(file, map))
                .transpose()?
        };
        if !is_declaration {
            self.cache.record_file_output(
                &file.relative_path,
                &obfuscated_relative_path,
                identifier_cache,
                member_method_cache,
            );
        }
        Ok(NativeSourceOutput {
            input_path: file.build_path.clone(),
            output_path,
            source_map_key: file.source_map_key.clone(),
            obfuscated_source_map_key: logical_path.rewrite(&file.source_map_key),
            record_name: file.record_name.clone(),
            obfuscated_record_name: file
                .record_name
                .as_deref()
                .map(|record_name| logical_path.rewrite(record_name)),
            code,
            source_map,
        })
    }

    fn plan_symbol_names(
        &mut self,
        file: &ObfuscationModuleFile,
        semantic: &oxc_semantic::Semantic<'_>,
        policy: &IdentifierPolicy,
    ) -> (Vec<SymbolRename>, BTreeMap<String, String>) {
        let scoping = semantic.scoping();
        let root_scope = scoping.root_scope_id();
        let symbol_ids = scoping.symbol_ids().collect::<Vec<_>>();
        let history = self.cache.identifier_history(file.source_name_cache_key());
        let mut cache = BTreeMap::new();
        let mut plan = Vec::new();
        for symbol_id in symbol_ids {
            let original = scoping.symbol_name(symbol_id).to_string();
            let scope_id = scoping.symbol_scope_id(symbol_id);
            let is_top_level = scope_id == root_scope;
            let flags = scoping.symbol_flags(symbol_id);
            let keep = self.reserved_names.contains(&original)
                || ArkguardWildcard::matches_any(&self.rules.keep_global_names, &original, false)
                || (!self.rules.enable_toplevel_obfuscation && is_top_level)
                || (!self.rules.enable_export_obfuscation
                    && (policy.exported_names.contains(&original)
                        || flags.intersects(SymbolFlags::Import | SymbolFlags::TypeImport)))
                || (self.rules.keep_parameter_names && policy.parameters.contains(&symbol_id));
            if keep {
                // With export obfuscation enabled ArkGuard still runs imported
                // bindings through its name generator. SDK-reserved imports
                // therefore produce identity entries in IdentifierCache.
                // See `RenameIdentifierTransformer.renames`.
                if is_top_level
                    && self.rules.enable_export_obfuscation
                    && flags.intersects(SymbolFlags::Import | SymbolFlags::TypeImport)
                {
                    cache.insert(format!("#{original}"), original);
                }
                continue;
            }
            let key = if is_top_level {
                format!("#{original}")
            } else {
                format!("{scope_id:?}#{original}")
            };
            let cached = Self::history_name(&history, &key, &original);
            let replacement = if is_top_level {
                self.cache
                    .property_cache
                    .get(&original)
                    .cloned()
                    .or(cached)
                    .unwrap_or_else(|| {
                        self.identifier_names.next_available(
                            &original,
                            &self.reserved_names,
                            &self.rules.keep_global_names,
                        )
                    })
            } else {
                cached.unwrap_or_else(|| {
                    self.identifier_names.next_available(
                        &original,
                        &self.reserved_names,
                        &self.rules.keep_global_names,
                    )
                })
            };
            self.reserved_names.insert(replacement.clone());
            if is_top_level {
                self.reserved_properties.insert(replacement.clone());
                self.cache
                    .property_cache
                    .insert(original.clone(), replacement.clone());
            }
            cache.insert(key, replacement.clone());
            plan.push(SymbolRename {
                declaration_start: scoping.symbol_span(symbol_id).start,
                original,
                replacement,
            });
        }
        (plan, cache)
    }

    fn history_name(
        history: &BTreeMap<String, String>,
        key: &str,
        original: &str,
    ) -> Option<String> {
        if let Some(name) = history.get(key) {
            return Some(name.clone());
        }
        let suffix = format!("#{original}");
        let mut matches = history
            .iter()
            .filter(|(candidate, _)| candidate.ends_with(&suffix))
            .map(|(_, name)| name);
        let name = matches.next()?.clone();
        matches.next().is_none().then_some(name)
    }

    fn apply_symbol_names(
        &self,
        scoping: &mut Scoping,
        plan: &[SymbolRename],
    ) -> ArkguardSyntheticSymbolSpans {
        let by_declaration = plan
            .iter()
            .map(|rename| {
                (
                    (rename.declaration_start, rename.original.as_str()),
                    rename.replacement.as_str(),
                )
            })
            .collect::<FxHashMap<_, _>>();
        let mut synthetic = ArkguardSyntheticSymbolSpans::default();
        let symbol_ids = scoping.symbol_ids().collect::<Vec<_>>();
        for symbol_id in symbol_ids {
            let span = scoping.symbol_span(symbol_id);
            let original = scoping.symbol_name(symbol_id);
            if let Some(replacement) = by_declaration.get(&(span.start, original)) {
                // RenameIdentifierTransformer.updateNameNode replaces every
                // renamed declaration identifier with factory.createIdentifier.
                // TypeScript factory-created nodes carry synthesized positions,
                // so the printer emits mappings only from their source-backed
                // parent declarations.
                synthetic.symbols.insert(symbol_id);
                synthetic.references.extend(
                    scoping
                        .get_resolved_reference_ids(symbol_id)
                        .iter()
                        .copied(),
                );
                scoping.set_symbol_name(symbol_id, Ident::from(*replacement));
            }
        }
        synthetic
    }

    fn plan_property_names(&mut self, names: Vec<String>) -> FxHashMap<String, String> {
        let mut output = FxHashMap::default();
        if !self.rules.enable_property_obfuscation {
            return output;
        }
        for original in names {
            if self.reserved_properties.contains(&original)
                || ArkguardWildcard::matches_any(&self.rules.keep_property_names, &original, false)
            {
                continue;
            }
            let replacement = self
                .cache
                .property_cache
                .get(&original)
                .cloned()
                .unwrap_or_else(|| {
                    self.identifier_names.next_available(
                        &original,
                        &self.reserved_properties,
                        &self.rules.keep_property_names,
                    )
                });
            self.reserved_properties.insert(replacement.clone());
            self.cache
                .property_cache
                .insert(original.clone(), replacement.clone());
            output.insert(original, replacement);
        }
        output
    }

    fn plan_file_names(&mut self, files: &[ObfuscationModuleFile]) {
        if !self.rules.enable_file_name_obfuscation {
            return;
        }
        for file in files {
            if !file.rename_file {
                continue;
            }
            let mut segments = file
                .relative_path
                .split('/')
                .map(str::to_string)
                .collect::<Vec<_>>();
            let last = segments.len().saturating_sub(1);
            for (index, segment) in segments.iter_mut().enumerate() {
                if matches!(segment.as_str(), "" | "." | "..") {
                    continue;
                }
                if index == last {
                    let (stem, extension) = ArkguardFileName::split(segment);
                    *segment = format!("{}{extension}", self.mangle_file_name_part(stem));
                } else {
                    *segment = self.mangle_file_name_part(segment);
                }
            }
            self.obfuscated_paths
                .insert(file.relative_path.clone(), segments.join("/"));
        }
    }

    fn record_name_mappings(&self, files: &[ObfuscationModuleFile]) -> BTreeMap<String, String> {
        files
            .iter()
            .filter_map(|file| {
                let record_name = file.record_name.as_ref()?;
                let obfuscated_relative_path = self.obfuscated_relative_path(&file.relative_path);
                let obfuscated =
                    ArkguardLogicalFilePath::new(&file.relative_path, &obfuscated_relative_path)
                        .rewrite(record_name);
                (record_name != &obfuscated).then(|| (record_name.clone(), obfuscated))
            })
            .collect()
    }

    fn obfuscated_relative_path(&self, relative_path: &str) -> String {
        self.obfuscated_paths
            .get(relative_path)
            .cloned()
            .unwrap_or_else(|| relative_path.to_string())
    }

    fn mangle_file_name_part(&mut self, original: &str) -> String {
        if self.reserved_file_names.contains(original)
            || ArkguardWildcard::matches_any(&self.rules.keep_file_names, original, false)
        {
            return original.to_string();
        }
        let replacement = self
            .cache
            .file_name_cache
            .get(original)
            .cloned()
            .unwrap_or_else(|| {
                self.file_names.next_available(
                    original,
                    &self.reserved_file_names,
                    &self.rules.keep_file_names,
                )
            });
        self.reserved_file_names.insert(replacement.clone());
        self.cache
            .file_name_cache
            .insert(original.to_string(), replacement.clone());
        replacement
    }

    fn output_path(&self, file: &ObfuscationModuleFile, obfuscated_relative_path: &str) -> PathBuf {
        if obfuscated_relative_path == file.relative_path {
            return file.build_path.clone();
        }
        let original_segments = file.relative_path.split('/').collect::<Vec<_>>();
        let obfuscated_segments = obfuscated_relative_path.split('/').collect::<Vec<_>>();
        let Some(build_name) = file.build_path.file_name().and_then(|name| name.to_str()) else {
            return file.build_path.clone();
        };
        let (_, build_extension) = ArkguardFileName::split(build_name);
        let mut matched_parents = 0usize;
        let mut build_parent = file.build_path.parent();
        for original_segment in original_segments.iter().rev().skip(1) {
            let Some(parent) = build_parent else {
                break;
            };
            if parent.file_name().and_then(|name| name.to_str()) != Some(*original_segment) {
                break;
            }
            matched_parents += 1;
            build_parent = parent.parent();
        }
        let Some(mut output) = build_parent.map(Path::to_path_buf) else {
            return file.build_path.clone();
        };
        let parent_start = obfuscated_segments
            .len()
            .saturating_sub(1 + matched_parents);
        for segment in &obfuscated_segments[parent_start..obfuscated_segments.len() - 1] {
            output.push(segment);
        }
        let (stem, _) =
            ArkguardFileName::split(obfuscated_segments.last().copied().unwrap_or(build_name));
        output.join(format!("{stem}{build_extension}"))
    }

    fn previous_output_paths(&self, files: &[ObfuscationModuleFile]) -> BTreeSet<PathBuf> {
        files
            .iter()
            .filter_map(|file| {
                self.cache
                    .files
                    .get(&file.relative_path)
                    .map(|cached| self.output_path(file, &cached.obfuscated_name))
            })
            .collect()
    }

    fn compose_source_map(
        &self,
        file: &ObfuscationModuleFile,
        generated: &oxc_sourcemap::SourceMap<'_>,
    ) -> Result<Value, ObfuscationError> {
        let previous = file
            .previous_source_map
            .as_ref()
            .map(|value| OwnedSourceMap::from_json_string(&value.to_string()))
            .transpose()
            .map_err(|error| ObfuscationError::CodegenError(error.to_string()))?;
        let (owned, mapping_line_count) = if let Some(previous) = previous.as_ref() {
            let merged = ArkguardSourceMapMerger::new(previous.as_source_map(), generated).merge();
            (merged.source_map, Some(merged.mapping_line_count))
        } else {
            (generated.clone().into_owned(), None)
        };
        let mut value: Value = serde_json::from_str(&owned.to_json_string())?;
        if let Some(map) = value.as_object_mut() {
            if let (Some(mapping_line_count), Some(Value::String(mappings))) =
                (mapping_line_count, map.get_mut("mappings"))
            {
                let encoded_line_count = mappings.bytes().filter(|byte| *byte == b';').count() + 1;
                mappings.extend(std::iter::repeat_n(
                    ';',
                    mapping_line_count.saturating_sub(encoded_line_count),
                ));
            }
            map.insert(
                "file".to_string(),
                Value::String(
                    file.original_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&file.relative_path)
                        .to_string(),
                ),
            );
            map.insert(
                "sources".to_string(),
                Value::Array(vec![Value::String(file.relative_path.clone())]),
            );
            map.insert("sourceRoot".to_string(), Value::String(String::new()));
            map.insert(
                "entry-package-info".to_string(),
                Value::String(self.adapter.entry_package_info.clone()),
            );
        }
        Ok(value)
    }

    fn remap_member_method_cache(
        &self,
        file: &ObfuscationModuleFile,
        cache: BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, ObfuscationError> {
        let previous = file
            .previous_source_map
            .as_ref()
            .map(|value| OwnedSourceMap::from_json_string(&value.to_string()))
            .transpose()
            .map_err(|error| ObfuscationError::CodegenError(error.to_string()))?;
        let lookup = previous.as_ref().map(OwnedSourceMap::generate_lookup_table);
        let mut remapped = BTreeMap::new();
        for (key, value) in cache {
            let mut parts = key.split(':');
            let Some(name) = parts.next() else {
                continue;
            };
            let positions = parts
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
                .ok();
            let Some([start_line, start_column, end_line, end_column]) = positions.as_deref()
            else {
                remapped.insert(key, value);
                continue;
            };
            if let (Some(previous), Some(lookup)) = (previous.as_ref(), lookup.as_ref()) {
                let start = previous
                    .lookup_source_view_token(
                        lookup,
                        start_line.saturating_sub(1),
                        start_column.saturating_sub(1),
                    )
                    .filter(|token| token.get_source_id().is_some());
                let end = previous
                    .lookup_source_view_token(
                        lookup,
                        end_line.saturating_sub(1),
                        end_column.saturating_sub(1),
                    )
                    .filter(|token| token.get_source_id().is_some());
                if (start.is_none() || end.is_none()) && name == value {
                    continue;
                }
                let remapped_key = match (start, end) {
                    (Some(start), Some(end)) => format!(
                        "{name}:{}:{}",
                        start.get_src_line() + 1,
                        end.get_src_line() + 1
                    ),
                    _ => name.to_string(),
                };
                remapped.insert(remapped_key, value);
            } else {
                remapped.insert(format!("{name}:{start_line}:{end_line}"), value);
            }
        }
        Ok(remapped)
    }
}

/// Relates ArkGuard's project-relative source name to its mangled form. The
/// compiler source uses this same filename mapping for source-map keys,
/// ModuleMode records, and OHM import URLs.
struct ArkguardLogicalFilePath<'a> {
    original: &'a str,
    obfuscated: &'a str,
}

impl<'a> ArkguardLogicalFilePath<'a> {
    fn new(original: &'a str, obfuscated: &'a str) -> Self {
        Self {
            original,
            obfuscated,
        }
    }

    fn rewrite(&self, value: &str) -> String {
        if self.original == self.obfuscated {
            return value.to_string();
        }
        for (original, obfuscated) in self.path_variants() {
            let Some(start) = value.rfind(&original) else {
                continue;
            };
            let before = value[..start].chars().next_back();
            let after = value[start + original.len()..].chars().next();
            if before.is_some_and(|character| !matches!(character, '/' | ':' | '&' | '|'))
                || after.is_some_and(|character| !matches!(character, '.' | '&' | '|'))
            {
                continue;
            }
            let mut rewritten = String::with_capacity(
                value.len() + obfuscated.len().saturating_sub(original.len()),
            );
            rewritten.push_str(&value[..start]);
            rewritten.push_str(&obfuscated);
            rewritten.push_str(&value[start + original.len()..]);
            return rewritten;
        }
        value.to_string()
    }

    fn path_variants(&self) -> Vec<(String, String)> {
        let mut variants = vec![(
            Self::without_extension(self.original),
            Self::without_extension(self.obfuscated),
        )];
        if let (Some((_, original)), Some((_, obfuscated))) = (
            self.original.split_once('/'),
            self.obfuscated.split_once('/'),
        ) {
            variants.push((
                Self::without_extension(original),
                Self::without_extension(obfuscated),
            ));
        }
        if let (Some((original_prefix, original)), Some((obfuscated_prefix, obfuscated))) = (
            Self::after_source_root(self.original),
            Self::after_source_root(self.obfuscated),
        ) {
            let original = Self::without_extension(original);
            let obfuscated = Self::without_extension(obfuscated);
            variants.push((original.clone(), obfuscated.clone()));
            let original_module = original_prefix.rsplit('/').next().unwrap_or_default();
            let obfuscated_module = obfuscated_prefix.rsplit('/').next().unwrap_or_default();
            if !original_module.is_empty() && !obfuscated_module.is_empty() {
                variants.push((
                    format!("{original_module}/{original}"),
                    format!("{obfuscated_module}/{obfuscated}"),
                ));
            }
            let original_source_root =
                self.original[original_prefix.len()..].trim_start_matches('/');
            let obfuscated_source_root =
                self.obfuscated[obfuscated_prefix.len()..].trim_start_matches('/');
            variants.push((
                Self::without_extension(original_source_root),
                Self::without_extension(obfuscated_source_root),
            ));
        }
        variants.sort_by_key(|variant| std::cmp::Reverse(variant.0.len()));
        variants.dedup_by(|left, right| left.0 == right.0);
        variants
    }

    fn after_source_root(path: &str) -> Option<(&str, &str)> {
        ["/src/main/", "/src/ohosTest/"]
            .into_iter()
            .find_map(|marker| {
                let index = path.find(marker)?;
                let prefix_end = index + marker.len() - 1;
                Some((&path[..index], &path[prefix_end + 1..]))
            })
    }

    fn without_extension(path: &str) -> String {
        let Some((parent, name)) = path.rsplit_once('/') else {
            return ArkguardFileName::split(path).0.to_string();
        };
        let (stem, _) = ArkguardFileName::split(name);
        format!("{parent}/{stem}")
    }
}

struct NativeSourceOutput {
    input_path: PathBuf,
    output_path: PathBuf,
    source_map_key: String,
    obfuscated_source_map_key: String,
    record_name: Option<String>,
    obfuscated_record_name: Option<String>,
    code: String,
    source_map: Option<Value>,
}

/// ArkGuard's `SourceMapMergingUtil.SourceMapLink` deliberately drops a
/// current-stage segment when the selected previous-stage segment is unmapped.
/// Rolldown's general-purpose collapse keeps such a segment as an explicit
/// unmapped token, so source-mode obfuscation must use ArkGuard's algorithm.
struct ArkguardSourceMapMerger<'a> {
    previous: &'a oxc_sourcemap::SourceMap<'a>,
    current: &'a oxc_sourcemap::SourceMap<'a>,
}

impl<'a> ArkguardSourceMapMerger<'a> {
    fn new(
        previous: &'a oxc_sourcemap::SourceMap<'a>,
        current: &'a oxc_sourcemap::SourceMap<'a>,
    ) -> Self {
        Self { previous, current }
    }

    fn merge(&self) -> ArkguardMergedSourceMap {
        // `SourceMapLink.traceMappings` retains one output row for every row
        // in the current decoded map, even when tracing drops every segment on
        // a trailing row. OXC's token model does not store empty mapping rows,
        // so retain that source-owned row count alongside the merged tokens.
        let mapping_line_count = self
            .current
            .get_tokens()
            .last()
            .map_or(0, |token| token.get_dst_line() as usize + 1);
        let mut traced = Vec::with_capacity(self.current.get_tokens().len());
        for token in self.current.get_tokens() {
            if token.get_source_id() != Some(0) {
                continue;
            }
            let Some(previous) =
                self.trace_previous_segment(token.get_src_line(), token.get_src_col())
            else {
                continue;
            };
            if previous.get_source_id() != Some(0) {
                continue;
            }
            let name = previous
                .get_name_id()
                .and_then(|id| self.previous.get_name(id))
                .or_else(|| token.get_name_id().and_then(|id| self.current.get_name(id)));
            traced.push(ArkguardTracedSourceMapToken {
                dst_line: token.get_dst_line(),
                dst_col: token.get_dst_col(),
                src_line: previous.get_src_line(),
                src_col: previous.get_src_col(),
                name,
            });
        }

        let mut builder = SourceMapBuilder::default();
        if !traced.is_empty() {
            builder.set_source_and_content(self.previous.get_source(0).unwrap_or_default(), "");
        }
        for token in traced {
            let name_id = token.name.map(|name| builder.add_name(name));
            builder.add_token(
                token.dst_line,
                token.dst_col,
                token.src_line,
                token.src_col,
                Some(0),
                name_id,
            );
        }
        let mut map = builder.into_owned_sourcemap().into_inner();
        map.set_source_contents(map.get_sources().map(|_| None).collect::<Vec<_>>());
        ArkguardMergedSourceMap {
            source_map: map,
            mapping_line_count,
        }
    }

    /// Port of
    /// `arkguard/src/utils/SourceMapMergingUtil.ts::SourceMapLink.traceSegment`.
    /// Its convergence rule is intentionally not a greatest-lower-bound
    /// lookup: once the binary-search window contains one segment, that
    /// segment is selected even when its generated column differs from the
    /// requested column. An unmapped selected segment terminates the trace.
    fn trace_previous_segment(&self, line: u32, column: u32) -> Option<oxc_sourcemap::Token> {
        let tokens = self.previous.get_tokens().collect::<Vec<_>>();
        let line_start = tokens.partition_point(|token| token.get_dst_line() < line);
        let line_end = tokens.partition_point(|token| token.get_dst_line() <= line);
        let segments = tokens.get(line_start..line_end)?;
        if segments.is_empty() {
            return None;
        }

        let mut search_start = 0_isize;
        let mut search_end = segments.len() as isize - 1;
        while search_start <= search_end {
            let middle = (search_start + search_end) >> 1;
            let segment = &segments[middle as usize];
            let segment_column = segment.get_dst_col();
            if segment_column == column || search_start == search_end {
                return segment.get_source_id().is_some().then_some(*segment);
            }
            if segment_column > column {
                search_end = middle - 1;
            } else {
                search_start = middle + 1;
            }
        }
        None
    }
}

struct ArkguardMergedSourceMap {
    source_map: oxc_sourcemap::SourceMap<'static>,
    mapping_line_count: usize,
}

struct ArkguardTracedSourceMapToken<'a> {
    dst_line: u32,
    dst_col: u32,
    src_line: u32,
    src_col: u32,
    name: Option<&'a str>,
}

struct SymbolRename {
    declaration_start: u32,
    original: String,
    replacement: String,
}

#[derive(Default)]
struct OrderedNameGenerator {
    next: usize,
}

impl OrderedNameGenerator {
    fn next_available(
        &mut self,
        original: &str,
        reserved: &FxHashSet<String>,
        wildcard_rules: &FxHashSet<String>,
    ) -> String {
        loop {
            let index = self.next;
            self.next += 1;
            let letter = char::from(b'a' + (index % 26) as u8);
            let round = index / 26;
            let candidate = if round == 0 {
                letter.to_string()
            } else {
                format!("{letter}{round}")
            };
            if candidate != original
                && !reserved.contains(&candidate)
                && !ArkguardWildcard::matches_any(wildcard_rules, &candidate, false)
            {
                return candidate;
            }
        }
    }
}

#[derive(Default)]
struct ArkguardNameCache {
    files: BTreeMap<String, ArkguardFileCache>,
    property_cache: BTreeMap<String, String>,
    file_name_cache: BTreeMap<String, String>,
}

#[derive(Default)]
struct ArkguardFileCache {
    identifier_cache: BTreeMap<String, String>,
    member_method_cache: BTreeMap<String, String>,
    obfuscated_name: String,
}

impl ArkguardNameCache {
    fn load(
        options: &ObfuscationOptions,
        adapter: &ArkguardAdapterOptions,
        rules: &ObfuscationRules,
    ) -> Result<Self, ObfuscationError> {
        let configured_cache = rules
            .apply_name_cache
            .as_deref()
            .map(|configured| adapter.project_root.join(configured));
        let path = configured_cache
            .clone()
            .unwrap_or_else(|| options.cache_dir.join("nameCache.json"));
        if !path.is_file() {
            if configured_cache.is_some() {
                return Err(ObfuscationError::NameCacheNotFound(path));
            }
            return Ok(Self::default());
        }
        let value: Value = serde_json::from_slice(&fs_err::read(path)?)?;
        let Some(object) = value.as_object() else {
            return Err(ObfuscationError::CodegenError(
                "ArkGuard nameCache.json root must be an object".to_string(),
            ));
        };
        let property_cache = Self::string_map(object.get("PropertyCache"));
        let file_name_cache = Self::string_map(object.get("FileNameCache"));
        let mut files = BTreeMap::new();
        for (path, value) in object {
            if matches!(
                path.as_str(),
                "PropertyCache" | "FileNameCache" | "compileSdkVersion" | "entryPackageInfo"
            ) {
                continue;
            }
            let Some(file) = value.as_object() else {
                continue;
            };
            files.insert(
                path.clone(),
                ArkguardFileCache {
                    identifier_cache: Self::string_map(file.get("IdentifierCache")),
                    member_method_cache: Self::string_map(file.get("MemberMethodCache")),
                    obfuscated_name: file
                        .get("obfName")
                        .and_then(Value::as_str)
                        .unwrap_or(path)
                        .to_string(),
                },
            );
        }
        Ok(Self {
            files,
            property_cache,
            file_name_cache,
        })
    }

    fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
        value
            .and_then(Value::as_object)
            .map(|object| {
                object
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn all_mangled_names(&self) -> impl Iterator<Item = String> + '_ {
        self.property_cache
            .values()
            .chain(
                self.files
                    .values()
                    .flat_map(|file| file.identifier_cache.values()),
            )
            .cloned()
    }

    fn identifier_history(&self, path: &str) -> BTreeMap<String, String> {
        self.files
            .get(path)
            .map(|file| {
                file.identifier_cache
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.split_once(':').map_or(key.as_str(), |(scope, _)| scope),
                            value.clone(),
                        )
                    })
                    .map(|(key, value)| (key.to_string(), value))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn record_file_output(
        &mut self,
        path: &str,
        obfuscated_name: &str,
        identifier_cache: BTreeMap<String, String>,
        member_method_cache: BTreeMap<String, String>,
    ) {
        self.files.insert(
            path.to_string(),
            ArkguardFileCache {
                identifier_cache,
                member_method_cache,
                obfuscated_name: obfuscated_name.to_string(),
            },
        );
    }

    fn prune_deleted_files(
        &mut self,
        files: &[ObfuscationModuleFile],
        adapter: &ArkguardAdapterOptions,
    ) -> Result<BTreeSet<PathBuf>, ObfuscationError> {
        let live = files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<FxHashSet<_>>();
        let deleted = self
            .files
            .iter()
            .filter(|(path, _)| !live.contains(path.as_str()))
            .map(|(path, file)| (path.clone(), file.obfuscated_name.clone()))
            .collect::<Vec<_>>();
        let mut outputs = BTreeSet::new();
        for (path, obfuscated_name) in deleted {
            let relative = RelPath::parse(PathBuf::from(&obfuscated_name))
                .map_err(|_| ObfuscationError::UnsafeNameCachePath(obfuscated_name.clone()))?;
            let mut output = adapter.cache_path.join(relative.as_path());
            output.set_extension("js");
            outputs.insert(output);
            self.files.remove(&path);
        }
        Ok(outputs)
    }

    fn write(
        &self,
        options: &ObfuscationOptions,
        adapter: &ArkguardAdapterOptions,
        rules: &ObfuscationRules,
    ) -> Result<PathBuf, ObfuscationError> {
        let mut root = Map::new();
        for (path, file) in &self.files {
            root.insert(
                path.clone(),
                serde_json::json!({
                    "IdentifierCache": file.identifier_cache,
                    "MemberMethodCache": file.member_method_cache,
                    "obfName": file.obfuscated_name,
                }),
            );
        }
        root.insert(
            "compileSdkVersion".to_string(),
            Value::String(adapter.compile_sdk_version.clone()),
        );
        root.insert(
            "entryPackageInfo".to_string(),
            Value::String(adapter.entry_package_info.clone()),
        );
        if rules.enable_property_obfuscation || rules.enable_export_obfuscation {
            root.insert(
                "PropertyCache".to_string(),
                serde_json::to_value(&self.property_cache)?,
            );
        }
        if rules.enable_file_name_obfuscation {
            root.insert(
                "FileNameCache".to_string(),
                serde_json::to_value(&self.file_name_cache)?,
            );
        }
        let bytes = serde_json::to_vec_pretty(&Value::Object(root))?;
        let default_path = options.cache_dir.join("nameCache.json");
        fs_err::write(&default_path, &bytes)?;
        if let Some(configured) = rules.print_name_cache.as_deref() {
            let configured_path = adapter.project_root.join(configured);
            // `ConfigResolver.writeObfuscationNameCache` writes an explicit
            // `-print-namecache` path directly. A missing parent is therefore
            // an IO error rather than an implicitly created directory.
            fs_err::write(configured_path, &bytes)?;
        }
        if rules.print_kept_names {
            let path = rules
                .print_kept_names_path
                .as_deref()
                .map(|configured| adapter.project_root.join(configured))
                .unwrap_or_else(|| options.cache_dir.join("keptNames.json"));
            if let Some(parent) = path.parent() {
                fs_err::create_dir_all(parent)?;
            }
            fs_err::write(
                path,
                serde_json::to_vec_pretty(&serde_json::json!({
                    "globalNames": rules.keep_global_names.iter().collect::<BTreeSet<_>>(),
                    "propertyNames": rules.keep_property_names.iter().collect::<BTreeSet<_>>(),
                    "fileNames": rules.keep_file_names.iter().collect::<BTreeSet<_>>(),
                }))?,
            )?;
        }
        Ok(default_path)
    }
}

struct IdentifierPolicy {
    exported_names: FxHashSet<String>,
    parameters: FxHashSet<oxc_syntax::symbol::SymbolId>,
}

impl IdentifierPolicy {
    fn collect(program: &oxc_ast::ast::Program<'_>) -> Self {
        let mut collector = IdentifierPolicyCollector::default();
        collector.visit_program(program);
        Self {
            exported_names: collector.exported_names,
            parameters: collector.parameters,
        }
    }
}

#[derive(Default)]
struct IdentifierPolicyCollector {
    export_depth: usize,
    exported_names: FxHashSet<String>,
    parameters: FxHashSet<oxc_syntax::symbol::SymbolId>,
}

impl IdentifierPolicyCollector {
    fn insert_export_name(&mut self, name: &ModuleExportName<'_>) {
        match name {
            ModuleExportName::IdentifierName(name) => {
                self.exported_names.insert(name.name.to_string());
            }
            ModuleExportName::IdentifierReference(name) => {
                self.exported_names.insert(name.name.to_string());
            }
            ModuleExportName::StringLiteral(name) => {
                self.exported_names.insert(name.value.to_string());
            }
        }
    }
}

impl<'a> Visit<'a> for IdentifierPolicyCollector {
    fn visit_binding_identifier(&mut self, identifier: &oxc_ast::ast::BindingIdentifier<'a>) {
        if self.export_depth > 0 {
            self.exported_names.insert(identifier.name.to_string());
        }
    }

    fn visit_formal_parameter(&mut self, parameter: &FormalParameter<'a>) {
        self.parameters.extend(
            parameter
                .pattern
                .get_binding_identifiers()
                .into_iter()
                .filter_map(|identifier| identifier.symbol_id.get()),
        );
        walk::walk_formal_parameter(self, parameter);
    }

    fn visit_formal_parameter_rest(&mut self, parameter: &FormalParameterRest<'a>) {
        self.parameters.extend(
            parameter
                .rest
                .argument
                .get_binding_identifiers()
                .into_iter()
                .filter_map(|identifier| identifier.symbol_id.get()),
        );
        walk::walk_formal_parameter_rest(self, parameter);
    }

    fn visit_export_declaration(&mut self, declaration: &ExportDeclaration<'a>) {
        self.export_depth += 1;
        walk::walk_export_declaration(self, declaration);
        self.export_depth -= 1;
    }

    fn visit_export_default_declaration(&mut self, declaration: &ExportDefaultDeclaration<'a>) {
        self.export_depth += 1;
        walk::walk_export_default_declaration(self, declaration);
        self.export_depth -= 1;
    }

    fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'a>) {
        for specifier in &declaration.specifiers {
            self.insert_export_name(&specifier.local);
            self.insert_export_name(&specifier.exported);
        }
        self.export_depth += 1;
        walk::walk_export_named_declaration(self, declaration);
        self.export_depth -= 1;
    }
}

struct PropertyNameCollector {
    names: Vec<String>,
    seen: FxHashSet<String>,
    include_strings: bool,
    include_exports: bool,
}

struct ObjectPropertyKeepCollector {
    names: FxHashSet<String>,
}

impl ObjectPropertyKeepCollector {
    fn collect(program: &oxc_ast::ast::Program<'_>) -> FxHashSet<String> {
        let mut collector = Self {
            names: FxHashSet::default(),
        };
        collector.visit_program(program);
        collector.names
    }
}

impl<'a> Visit<'a> for ObjectPropertyKeepCollector {
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        match &property.key {
            PropertyKey::StaticIdentifier(identifier) => {
                self.names.insert(identifier.name.to_string());
            }
            PropertyKey::PrivateIdentifier(identifier) => {
                self.names.insert(identifier.name.to_string());
            }
            PropertyKey::StringLiteral(literal) => {
                self.names.insert(literal.value.to_string());
            }
            _ => {}
        }
        walk::walk_object_property(self, property);
    }
}

struct MemberMethodCacheCollector<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
    properties: &'a FxHashMap<String, String>,
    symbols: &'a [SymbolRename],
    classes: Vec<Option<(String, String, bool)>>,
    cache: BTreeMap<String, String>,
}

impl<'a> MemberMethodCacheCollector<'a> {
    fn new(
        source: &'a str,
        properties: &'a FxHashMap<String, String>,
        symbols: &'a [SymbolRename],
    ) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self {
            source,
            line_starts,
            properties,
            symbols,
            classes: Vec::new(),
            cache: BTreeMap::new(),
        }
    }

    fn collect(mut self, program: &oxc_ast::ast::Program<'_>) -> BTreeMap<String, String> {
        self.visit_program(program);
        self.cache
    }

    fn property_name<'p>(key: &'p PropertyKey<'_>) -> Option<&'p str> {
        match key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str()),
            PropertyKey::PrivateIdentifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        }
    }

    fn line_and_column(&self, offset: u32) -> (usize, usize) {
        let offset = usize::try_from(offset).unwrap_or(self.source.len());
        let line = self.line_starts.partition_point(|start| *start <= offset);
        let line_start = self.line_starts[line.saturating_sub(1)];
        let column = self.source[line_start..offset.min(self.source.len())]
            .encode_utf16()
            .count()
            + 1;
        (line, column)
    }

    fn span_suffix(&self, span: oxc_span::Span) -> String {
        let (start_line, start_column) = self.line_and_column(span.start);
        let (end_line, end_column) = self.line_and_column(span.end);
        format!(":{start_line}:{start_column}:{end_line}:{end_column}")
    }
}

impl<'a> Visit<'a> for MemberMethodCacheCollector<'_> {
    fn visit_class(&mut self, class: &Class<'a>) {
        let class_name = class.id.as_ref().map(|identifier| {
            let original = identifier.name.to_string();
            let replacement = self
                .symbols
                .iter()
                .find(|rename| {
                    rename.declaration_start == identifier.span.start && rename.original == original
                })
                .map(|rename| rename.replacement.clone())
                .unwrap_or_else(|| original.clone());
            let view_pu_based = class.heritage.as_ref().is_some_and(|heritage| {
                matches!(
                    &heritage.expression,
                    Expression::Identifier(identifier)
                        if matches!(identifier.name.as_str(), "ViewPU" | "ViewV2" | "PUV2ViewBase")
                )
            });
            (original, replacement, view_pu_based)
        });
        self.classes.push(class_name);
        walk::walk_class(self, class);
        self.classes.pop();
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        let names = if method.kind == MethodDefinitionKind::Constructor {
            self.classes
                .last()
                .cloned()
                .flatten()
                .filter(|(_, _, view_pu_based)| !view_pu_based)
                .map(|(original, replacement, _)| (original, replacement))
        } else {
            Self::property_name(&method.key).map(|original| {
                let replacement = self
                    .properties
                    .get(original)
                    .cloned()
                    .unwrap_or_else(|| original.to_string());
                (original.to_string(), replacement)
            })
        };
        if let Some((original, replacement)) = names {
            self.cache.insert(
                format!("{}{}", original, self.span_suffix(method.span)),
                replacement,
            );
        }
        walk::walk_method_definition(self, method);
    }
}

impl PropertyNameCollector {
    fn new(include_strings: bool, include_exports: bool) -> Self {
        Self {
            names: Vec::new(),
            seen: FxHashSet::default(),
            include_strings,
            include_exports,
        }
    }

    fn insert(&mut self, name: &str) {
        if self.seen.insert(name.to_string()) {
            self.names.push(name.to_string());
        }
    }

    fn insert_key(&mut self, key: &PropertyKey<'_>) {
        match key {
            PropertyKey::StaticIdentifier(identifier) => self.insert(identifier.name.as_str()),
            PropertyKey::PrivateIdentifier(identifier) => self.insert(identifier.name.as_str()),
            PropertyKey::StringLiteral(literal) if self.include_strings => {
                self.insert(literal.value.as_str());
            }
            _ => {}
        }
    }
}

impl<'a> Visit<'a> for PropertyNameCollector {
    fn visit_property_key(&mut self, key: &PropertyKey<'a>) {
        self.insert_key(key);
        walk::walk_property_key(self, key);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.insert(member.property.name.as_str());
        walk::walk_static_member_expression(self, member);
    }

    fn visit_private_field_expression(&mut self, member: &PrivateFieldExpression<'a>) {
        self.insert(member.field.name.as_str());
        walk::walk_private_field_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if self.include_strings {
            if let Expression::StringLiteral(literal) = &member.expression {
                self.insert(literal.value.as_str());
            }
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_module_export_name(&mut self, name: &ModuleExportName<'a>) {
        if self.include_exports {
            match name {
                ModuleExportName::IdentifierName(identifier) => {
                    self.insert(identifier.name.as_str());
                }
                ModuleExportName::StringLiteral(literal) => {
                    self.insert(literal.value.as_str());
                }
                ModuleExportName::IdentifierReference(_) => {}
            }
        }
        walk::walk_module_export_name(self, name);
    }
}

#[derive(Default)]
struct ArkguardSyntheticSymbolSpans {
    symbols: FxHashSet<SymbolId>,
    references: FxHashSet<ReferenceId>,
}

impl ArkguardSyntheticSymbolSpans {
    fn transform<'a>(&mut self, program: &mut Program<'a>) {
        self.visit_program(program);
    }
}

impl<'a> VisitMut<'a> for ArkguardSyntheticSymbolSpans {
    fn visit_binding_identifier(&mut self, identifier: &mut BindingIdentifier<'a>) {
        if identifier
            .symbol_id
            .get()
            .is_some_and(|symbol_id| self.symbols.contains(&symbol_id))
        {
            identifier.span = Span::default();
        }
    }

    fn visit_identifier_reference(&mut self, identifier: &mut IdentifierReference<'a>) {
        if identifier
            .reference_id
            .get()
            .is_some_and(|reference_id| self.references.contains(&reference_id))
        {
            identifier.span = Span::default();
        }
    }
}

struct NativeArkguardAstTransform<'a, 'm> {
    allocator: &'a Allocator,
    properties: &'m FxHashMap<String, String>,
    file_names: &'m BTreeMap<String, String>,
    record_names: &'m BTreeMap<String, String>,
    rename_module_paths: bool,
    rename_strings: bool,
    rename_exports: bool,
}

impl<'a, 'm> NativeArkguardAstTransform<'a, 'm> {
    fn new(
        allocator: &'a Allocator,
        properties: &'m FxHashMap<String, String>,
        file_names: &'m BTreeMap<String, String>,
        record_names: &'m BTreeMap<String, String>,
        rename_module_paths: bool,
        rename_strings: bool,
        rename_exports: bool,
    ) -> Self {
        Self {
            allocator,
            properties,
            file_names,
            record_names,
            rename_module_paths,
            rename_strings,
            rename_exports,
        }
    }

    fn transform(&mut self, program: &mut oxc_ast::ast::Program<'a>) {
        self.visit_program(program);
    }

    fn rename_identifier(&self, identifier: &mut IdentifierName<'a>) {
        if let Some(replacement) = self.properties.get(identifier.name.as_str()) {
            identifier.name = Ident::from_str_in(replacement, &self.allocator);
            identifier.span = Span::default();
        }
    }

    fn rename_string(&self, literal: &mut StringLiteral<'a>) {
        if let Some(replacement) = self.properties.get(literal.value.as_str()) {
            literal.value = Str::from_str_in(replacement, &self.allocator);
            literal.raw = None;
            literal.span = Span::default();
        }
    }

    fn rename_module_specifier(&self, literal: &mut StringLiteral<'a>) {
        let replacement = ArkguardModuleSpecifier::new(
            self.file_names,
            self.record_names,
            self.rename_module_paths,
        )
        .rewrite(literal.value.as_str());
        if literal.value != replacement {
            literal.value = Str::from_str_in(&replacement, &self.allocator);
            literal.raw = None;
            literal.span = Span::default();
        }
    }
}

impl<'a> VisitMut<'a> for NativeArkguardAstTransform<'a, '_> {
    fn visit_object_property(&mut self, property: &mut ObjectProperty<'a>) {
        let original = match &property.key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
            PropertyKey::StringLiteral(literal) if self.rename_strings => {
                Some(literal.value.to_string())
            }
            _ => None,
        };
        if original
            .as_ref()
            .is_some_and(|name| self.properties.contains_key(name))
        {
            property.shorthand = false;
        }
        walk_mut::walk_object_property(self, property);
    }

    fn visit_property_key(&mut self, key: &mut PropertyKey<'a>) {
        match key {
            PropertyKey::StaticIdentifier(identifier) => self.rename_identifier(identifier),
            PropertyKey::PrivateIdentifier(identifier) => {
                if let Some(replacement) = self.properties.get(identifier.name.as_str()) {
                    identifier.name = Ident::from_str_in(replacement, &self.allocator);
                    identifier.span = Span::default();
                }
            }
            PropertyKey::StringLiteral(literal) if self.rename_strings => {
                self.rename_string(literal);
            }
            _ => walk_mut::walk_property_key(self, key),
        }
    }

    fn visit_static_member_expression(&mut self, member: &mut StaticMemberExpression<'a>) {
        self.rename_identifier(&mut member.property);
        walk_mut::walk_expression(self, &mut member.object);
    }

    fn visit_private_field_expression(&mut self, member: &mut PrivateFieldExpression<'a>) {
        if let Some(replacement) = self.properties.get(member.field.name.as_str()) {
            member.field.name = Ident::from_str_in(replacement, &self.allocator);
            member.field.span = Span::default();
        }
        walk_mut::walk_expression(self, &mut member.object);
    }

    fn visit_computed_member_expression(&mut self, member: &mut ComputedMemberExpression<'a>) {
        if self.rename_strings {
            if let Expression::StringLiteral(literal) = &mut member.expression {
                self.rename_string(literal);
            }
        }
        walk_mut::walk_computed_member_expression(self, member);
    }

    fn visit_import_declaration(&mut self, declaration: &mut ImportDeclaration<'a>) {
        self.rename_module_specifier(&mut declaration.source);
        walk_mut::walk_import_declaration(self, declaration);
    }

    fn visit_lazy_import_declaration(&mut self, declaration: &mut LazyImportDeclaration<'a>) {
        self.rename_module_specifier(&mut declaration.source);
        walk_mut::walk_lazy_import_declaration(self, declaration);
    }

    fn visit_export_from_declaration(&mut self, declaration: &mut ExportFromDeclaration<'a>) {
        self.rename_module_specifier(&mut declaration.source);
        walk_mut::walk_export_from_declaration(self, declaration);
    }

    fn visit_export_all_declaration(&mut self, declaration: &mut ExportAllDeclaration<'a>) {
        self.rename_module_specifier(&mut declaration.source);
        walk_mut::walk_export_all_declaration(self, declaration);
    }

    fn visit_import_expression(&mut self, expression: &mut ImportExpression<'a>) {
        if let Expression::StringLiteral(literal) = &mut expression.source {
            self.rename_module_specifier(literal);
        }
        walk_mut::walk_import_expression(self, expression);
    }

    fn visit_module_export_name(&mut self, name: &mut ModuleExportName<'a>) {
        if self.rename_exports {
            match name {
                ModuleExportName::IdentifierName(identifier) => {
                    self.rename_identifier(identifier);
                    return;
                }
                ModuleExportName::StringLiteral(literal) => {
                    self.rename_string(literal);
                    return;
                }
                ModuleExportName::IdentifierReference(_) => {}
            }
        }
        walk_mut::walk_module_export_name(self, name);
    }
}

struct ExpressionStatementRemover<'source, 'rules> {
    source: &'source str,
    rules: &'rules ObfuscationRules,
}

impl<'source, 'rules> ExpressionStatementRemover<'source, 'rules> {
    fn new(source: &'source str, rules: &'rules ObfuscationRules) -> Self {
        Self { source, rules }
    }

    fn transform<'a>(&mut self, program: &mut oxc_ast::ast::Program<'a>) {
        self.visit_program(program);
    }

    fn is_removable_statement(&self, statement: &Statement<'_>) -> bool {
        let Statement::ExpressionStatement(statement) = statement else {
            return false;
        };
        let Some(call) = Self::call_expression(&statement.expression) else {
            return false;
        };
        if self.rules.remove_log && Self::is_console_callee(&call.callee) {
            return true;
        }
        if self.rules.remove_no_side_effect_calls.is_empty()
            || !Self::is_property_or_element_access_chain(&call.callee)
        {
            return false;
        }
        let span = call.callee.span();
        let Some(name) = self.source.get(span.start as usize..span.end as usize) else {
            return false;
        };
        self.rules.remove_no_side_effect_calls.contains(name)
            || ArkguardWildcard::matches_any(&self.rules.remove_no_side_effect_calls, name, false)
    }

    fn call_expression<'ast>(
        expression: &'ast Expression<'_>,
    ) -> Option<&'ast oxc_ast::ast::CallExpression<'ast>> {
        match expression {
            Expression::CallExpression(call) => Some(call),
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::CallExpression(call) => Some(call),
                _ => None,
            },
            _ => None,
        }
    }

    fn is_console_callee(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::StaticMemberExpression(member) => {
                matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "console")
            }
            Expression::ComputedMemberExpression(member) => {
                matches!(&member.object, Expression::Identifier(identifier) if identifier.name == "console")
            }
            _ => false,
        }
    }

    fn is_property_or_element_access_chain(expression: &Expression<'_>) -> bool {
        match expression {
            Expression::Identifier(_) => true,
            Expression::StaticMemberExpression(member) => {
                Self::is_property_or_element_access_chain(&member.object)
            }
            Expression::ComputedMemberExpression(member) => {
                Self::is_property_or_element_access_chain(&member.object)
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::StaticMemberExpression(member) => {
                    Self::is_property_or_element_access_chain(&member.object)
                }
                ChainElement::ComputedMemberExpression(member) => {
                    Self::is_property_or_element_access_chain(&member.object)
                }
                _ => false,
            },
            _ => false,
        }
    }
}

impl<'a> VisitMut<'a> for ExpressionStatementRemover<'_, '_> {
    fn visit_statements(&mut self, statements: &mut oxc_allocator::Vec<'a, Statement<'a>>) {
        for statement in statements.iter_mut() {
            self.visit_statement(statement);
        }
        statements.retain(|statement| !self.is_removable_statement(statement));
    }
}

struct ArkguardFileName;

impl ArkguardFileName {
    fn split(name: &str) -> (&str, &str) {
        if let Some(stem) = name.strip_suffix(".d.ets") {
            return (stem, ".d.ets");
        }
        if let Some(stem) = name.strip_suffix(".d.ts") {
            return (stem, ".d.ts");
        }
        let Some(index) = name.rfind('.') else {
            return (name, "");
        };
        (&name[..index], &name[index..])
    }
}

struct ArkguardModuleSpecifier<'a> {
    file_names: &'a BTreeMap<String, String>,
    record_names: &'a BTreeMap<String, String>,
    rename_relative_paths: bool,
}

impl<'a> ArkguardModuleSpecifier<'a> {
    fn new(
        file_names: &'a BTreeMap<String, String>,
        record_names: &'a BTreeMap<String, String>,
        rename_relative_paths: bool,
    ) -> Self {
        Self {
            file_names,
            record_names,
            rename_relative_paths,
        }
    }

    fn rewrite(&self, original: &str) -> String {
        let specifier = ModuleSpecifierText::new(original);
        if specifier.kind() == SpecifierKind::OhmUrl {
            for (record_name, obfuscated_record_name) in self.record_names {
                if let Some(prefix) = original.strip_suffix(record_name) {
                    return format!("{prefix}{obfuscated_record_name}");
                }
            }
            return original.to_string();
        }
        if !self.rename_relative_paths {
            return original.to_string();
        }
        if !matches!(
            specifier.kind(),
            SpecifierKind::Relative | SpecifierKind::AbsolutePath
        ) {
            return original.to_string();
        }
        let Ok(normalized) = ArtifactPathSerializer::path_to_slash_string(Path::new(original))
        else {
            return original.to_string();
        };
        let mut segments = normalized
            .split('/')
            .map(str::to_string)
            .collect::<Vec<_>>();
        let last = segments.len().saturating_sub(1);
        for (index, segment) in segments.iter_mut().enumerate() {
            if matches!(segment.as_str(), "" | "." | "..") {
                continue;
            }
            if index == last {
                let (stem, extension) = ArkguardFileName::split(segment);
                if let Some(mangled) = self.file_names.get(stem) {
                    *segment = format!("{mangled}{extension}");
                }
            } else if let Some(mangled) = self.file_names.get(segment) {
                *segment = mangled.clone();
            }
        }
        segments.join("/")
    }
}

impl ObfuscationRules {
    fn keeps_source(&self, original_path: &Path, _relative_path: &str) -> bool {
        if self.keep_source_paths.is_empty() {
            return false;
        }
        let absolute = canonicalize_or_owned(original_path);
        let Ok(serialized) = ArtifactPathSerializer::path_to_slash_string(&absolute) else {
            return false;
        };
        if self.exclude_source_paths.iter().any(|configured| {
            configured == &serialized
                || ArkguardWildcard::new(configured, true).matches(&serialized)
        }) {
            return false;
        }
        self.keep_source_paths.iter().any(|configured| {
            if ArkguardWildcard::new(configured, true).matches(&serialized) {
                return true;
            }
            let configured_path = Path::new(configured);
            configured == &serialized
                || (configured_path.is_dir() && absolute.starts_with(configured_path))
        })
    }

    fn compacts(&self, original_path: &Path) -> bool {
        if !self.compact || self.keep_uncompact_paths.is_empty() {
            return self.compact;
        }
        let absolute = canonicalize_or_owned(original_path);
        let Ok(serialized) = ArtifactPathSerializer::path_to_slash_string(&absolute) else {
            return self.compact;
        };
        if self.exclude_uncompact_paths.iter().any(|configured| {
            configured == &serialized
                || ArkguardWildcard::new(configured, true).matches(&serialized)
        }) {
            return true;
        }
        !self.keep_uncompact_paths.iter().any(|configured| {
            if ArkguardWildcard::new(configured, true).matches(&serialized) {
                return true;
            }
            let configured_path = Path::new(configured);
            configured == &serialized
                || (configured_path.is_dir() && absolute.starts_with(configured_path))
        })
    }
}

struct DeclarationCommentTargetCollector<'rules> {
    rules: &'rules ObfuscationRules,
    starts: BTreeSet<u32>,
}

impl<'rules> DeclarationCommentTargetCollector<'rules> {
    fn new(rules: &'rules ObfuscationRules) -> Self {
        Self {
            rules,
            starts: BTreeSet::new(),
        }
    }

    fn collect<'ast>(mut self, program: &oxc_ast::ast::Program<'ast>) -> BTreeSet<u32> {
        self.visit_program(program);
        self.starts
    }

    fn keeps(&self, name: &str) -> bool {
        self.rules.keep_comments.contains(name)
            || ArkguardWildcard::matches_any(&self.rules.keep_comments, name, false)
    }

    fn keep_key(&mut self, start: u32, key: &PropertyKey<'_>) {
        let name = match key {
            PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str()),
            PropertyKey::StringLiteral(literal) => Some(literal.value.as_str()),
            PropertyKey::PrivateIdentifier(identifier) => Some(identifier.name.as_str()),
            _ => None,
        };
        if name.is_some_and(|name| self.keeps(name)) {
            self.starts.insert(start);
        }
    }

    fn keeps_attached_comment(source: &str, starts: &BTreeSet<u32>, attached_to: u32) -> bool {
        starts.iter().any(|start| {
            if *start == attached_to {
                return true;
            }
            if *start < attached_to {
                return false;
            }
            source
                .get(attached_to as usize..*start as usize)
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
}

impl<'ast> Visit<'ast> for DeclarationCommentTargetCollector<'_> {
    fn visit_class(&mut self, class: &Class<'ast>) {
        if class
            .id
            .as_ref()
            .is_some_and(|identifier| self.keeps(identifier.name.as_str()))
        {
            self.starts.insert(class.span.start);
        }
        walk::walk_class(self, class);
    }

    fn visit_function(&mut self, function: &Function<'ast>, flags: oxc_syntax::scope::ScopeFlags) {
        if function
            .id
            .as_ref()
            .is_some_and(|identifier| self.keeps(identifier.name.as_str()))
        {
            self.starts.insert(function.span.start);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'ast>) {
        if declaration.declarations.iter().any(|declarator| {
            declarator
                .id
                .get_binding_identifiers()
                .iter()
                .any(|identifier| self.keeps(identifier.name.as_str()))
        }) {
            self.starts.insert(declaration.span.start);
        }
        walk::walk_variable_declaration(self, declaration);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'ast>) {
        self.keep_key(method.span.start, &method.key);
        walk::walk_method_definition(self, method);
    }

    fn visit_property_definition(&mut self, property: &PropertyDefinition<'ast>) {
        self.keep_key(property.span.start, &property.key);
        walk::walk_property_definition(self, property);
    }

    fn visit_accessor_property(&mut self, property: &AccessorProperty<'ast>) {
        self.keep_key(property.span.start, &property.key);
        walk::walk_accessor_property(self, property);
    }

    fn visit_ts_interface_declaration(&mut self, interface: &TSInterfaceDeclaration<'ast>) {
        if self.keeps(interface.id.name.as_str()) {
            self.starts.insert(interface.span.start);
        }
        walk::walk_ts_interface_declaration(self, interface);
    }

    fn visit_ts_property_signature(&mut self, property: &TSPropertySignature<'ast>) {
        self.keep_key(property.span.start, &property.key);
        walk::walk_ts_property_signature(self, property);
    }

    fn visit_ts_method_signature(&mut self, method: &TSMethodSignature<'ast>) {
        self.keep_key(method.span.start, &method.key);
        walk::walk_ts_method_signature(self, method);
    }

    fn visit_ts_enum_declaration(&mut self, declaration: &TSEnumDeclaration<'ast>) {
        if self.keeps(declaration.id.name.as_str()) {
            self.starts.insert(declaration.span.start);
        }
        for member in &declaration.body.members {
            let name = match &member.id {
                TSEnumMemberName::Identifier(identifier) => Some(identifier.name.as_str()),
                TSEnumMemberName::String(literal) | TSEnumMemberName::ComputedString(literal) => {
                    Some(literal.value.as_str())
                }
                TSEnumMemberName::ComputedTemplateString(_) => None,
            };
            if name.is_some_and(|name| self.keeps(name)) {
                self.starts.insert(member.span.start);
            }
        }
        walk::walk_ts_enum_declaration(self, declaration);
    }
}

/// ArkGuard wildcard matcher. Name patterns use `*`/`?` over all characters;
/// path patterns additionally make a single `*`/`?` stop at `/`, while `**`
/// crosses path separators (`TransformUtil.wildcardTransformer`).
pub(crate) struct ArkguardWildcard<'a> {
    pattern: &'a str,
    path: bool,
}

impl<'a> ArkguardWildcard<'a> {
    pub(crate) fn new(pattern: &'a str, path: bool) -> Self {
        Self { pattern, path }
    }

    fn matches_any<'pattern>(
        patterns: &'pattern FxHashSet<String>,
        value: &str,
        path: bool,
    ) -> bool {
        patterns
            .iter()
            .filter(|pattern| pattern.contains(['*', '?']))
            .any(|pattern| ArkguardWildcard::<'pattern>::new(pattern, path).matches(value))
    }

    pub(crate) fn matches(&self, value: &str) -> bool {
        if !self.pattern.contains(['*', '?']) {
            return self.pattern == value;
        }
        let pattern = self.pattern.chars().collect::<Vec<_>>();
        let value = value.chars().collect::<Vec<_>>();
        let mut states = FxHashSet::default();
        let mut stack = vec![(0usize, 0usize)];
        while let Some((pattern_index, value_index)) = stack.pop() {
            if !states.insert((pattern_index, value_index)) {
                continue;
            }
            if pattern_index == pattern.len() {
                if value_index == value.len() {
                    return true;
                }
                continue;
            }
            match pattern[pattern_index] {
                '*' => {
                    let double = pattern.get(pattern_index + 1) == Some(&'*');
                    let next_pattern = pattern_index + if double { 2 } else { 1 };
                    stack.push((next_pattern, value_index));
                    if value_index < value.len()
                        && (double || !self.path || value[value_index] != '/')
                    {
                        stack.push((pattern_index, value_index + 1));
                    }
                }
                '?' if value_index < value.len() && (!self.path || value[value_index] != '/') => {
                    stack.push((pattern_index + 1, value_index + 1));
                }
                character if value.get(value_index) == Some(&character) => {
                    stack.push((pattern_index + 1, value_index + 1));
                }
                _ => {}
            }
        }
        false
    }

    pub(crate) fn regex_string(pattern: &str) -> String {
        let mut body = String::with_capacity(pattern.len().saturating_mul(2));
        for character in pattern.chars() {
            match character {
                '*' => body.push_str(".*"),
                '?' => body.push('.'),
                '\\' | '+' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '.' => {
                    body.push('\\');
                    body.push(character);
                }
                _ => body.push(character),
            }
        }
        format!("/^{body}$/")
    }
}
