//! In-process ArkTS compiler. tsrs owns snapshots, checking and immutable facts.
mod filesystem;
mod options;
mod plugins;

use filesystem::SourceSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use tsrs_api::embedded::Compiler;
use tsrs_vfs::{FS, bundled, osvfs};

pub use tsrs_api::embedded::{
    AnnotationConstant as NativeAnnotationConstant, AnnotationInfo as NativeAnnotationInfo,
    AnnotationProperty as NativeAnnotationProperty,
    AnnotationTransformImport as NativeAnnotationTransformImport,
    AnnotationTransformInfo as NativeAnnotationTransformInfo,
    AnnotationTransformNode as NativeAnnotationTransformNode,
    ArktsBuilderReceiverTypeFacts as NativeArktsBuilderReceiverTypeFacts,
    ArktsNormalDiagnostics as NativeArktsNormalDiagnostics,
    ArktsPropertyTypeFacts as NativeArktsPropertyTypeFacts,
    ArktsTransformTypeFacts as NativeArktsTransformTypeFacts,
    ArktsTransformTypeQuery as NativeArktsTransformTypeQuery, ProgramSnapshot as NativeProgram,
    ProgramSourceGraph as NativeProgramSourceGraph,
    ProgramSourceGraphFile as NativeProgramSourceGraphFile,
    ResolvedTypeIdentity as NativeResolvedTypeIdentity, Symbol as NativeSymbol,
    TypeHandle as NativeType, TypeQueryRange as NativeTypeQueryRange,
};

#[derive(Debug, thiserror::Error)]
pub enum NativeCheckError {
    #[error("ARKDOWN_TYPE_SERVICE: {0}")]
    Compiler(String),
    #[error("ARKDOWN_TYPE_SERVICE: {0}")]
    Api(#[from] tsrs_api::ApiError),
    #[error("ARKDOWN_TYPE_SERVICE_PATH: {0}")]
    Path(#[from] arkdown_path::serializer::SerializerError),
    #[error("ARKDOWN_TYPE_SERVICE_IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("ARKDOWN_TYPE_SERVICE_JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Native tsrs compiler options (enum numbers, not tsconfig strings).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeCompilerOptions {
    pub ets: Arc<arkdown_ohos_schema::ets_options::EtsOptions>,
    pub ets_loader_path: PathBuf,
    pub compile_sdk_version: u32,
    pub target: u32,
    pub module: u32,
    pub module_resolution: u32,
    pub lib: Vec<String>,
    pub types: Vec<String>,
    pub type_roots: Vec<PathBuf>,
    pub paths: BTreeMap<String, Vec<PathBuf>>,
    pub paths_base_path: PathBuf,
    pub base_url: PathBuf,
    // ets_checker.ts::setCompilerOptions sets noEmit. Keep this private:
    // callers may configure checking, but cannot delegate build output to TS.
    no_emit: bool,
    // Explicit session checker count, independent of the compiler default.
    checkers: usize,
    pub incremental: bool,
    pub ts_build_info_file: PathBuf,
    pub skip_paths_in_key_for_compilation_settings: bool,
    pub skip_base_url_in_key_for_compilation_settings: bool,
    pub allow_js: bool,
    pub check_js: bool,
    pub emit_node_modules_files: bool,
    pub allow_synthetic_default_imports: bool,
    pub es_module_interop: bool,
    pub experimental_decorators: bool,
    pub resolve_json_module: bool,
    pub skip_lib_check: bool,
    pub strict: bool,
    pub strict_null_checks: bool,
    pub always_strict: bool,
    pub is_compile_js_har: bool,
    pub module_root_path: PathBuf,
    pub max_flow_depth: f64,
    pub ets_annotations_enable: bool,
    pub need_do_ark_ts_linter: bool,
    pub is_compatible_version: bool,
    pub ts_import_sendable_enable: bool,
    pub ts_import_so_check: bool,
    pub package_manager_type: String,
    pub skip_tsc_oh_module_check: bool,
    #[serde(rename = "skipArkTSStaticBlocksCheck")]
    pub skip_ark_ts_static_blocks_check: bool,
    pub compatible_sdk_version: Option<f64>,
    pub compatible_sdk_version_stage: String,
    pub skip_oh_modules_lint: bool,
    #[serde(rename = "enableStrictCheckOHModule")]
    pub enable_strict_check_oh_module: bool,
    pub disable_strict_check_paths: Vec<PathBuf>,
    pub disable_sendable_check_rules: Vec<String>,
    pub mix_compile: bool,
    pub strict_checker_only: bool,
    pub oh_sdk_configs: Vec<NativeOhSdkConfig>,
    pub oh_system_modules: Vec<String>,
    pub oh_sdk_config_prefixes: Vec<String>,
    pub oh_fallback_module_roots: Vec<PathBuf>,
    pub oh_loader_module_root: PathBuf,
    pub oh_project_path: PathBuf,
    pub oh_external_api_paths: Vec<PathBuf>,
    pub oh_package_exports: BTreeMap<String, Vec<PathBuf>>,
    #[serde(rename = "ohRuntimeOS")]
    pub oh_runtime_os: String,
    pub oh_origin_compatible_sdk_version: String,
    pub oh_project_root_path: PathBuf,
    pub oh_module_path: PathBuf,
    pub oh_all_module_paths: Vec<PathBuf>,
    pub oh_global_module_paths: Vec<PathBuf>,
    #[serde(rename = "ohArkUIDeclarationDirs")]
    pub oh_ark_ui_declaration_dirs: Vec<PathBuf>,
    pub oh_request_permissions: Vec<String>,
    pub oh_syscap_intersection: Vec<String>,
    pub oh_syscap_union: Vec<String>,
    pub oh_device_types: Vec<String>,
    pub oh_card_entry_files: Vec<PathBuf>,
    pub oh_crossplatform: bool,
    pub oh_ignore_crossplatform_check: bool,
    pub oh_compile_mode: String,
    pub oh_bundle_type: String,
    pub oh_api_compatibility_check: String,
    pub oh_sdk_check_plugins: Vec<NativeOhSdkCheckPlugin>,
    pub oh_sdk_class_check_plugins: Vec<NativeOhSdkClassCheckPlugin>,
    /// Complete source-owned `projectConfig` passed to SDK class checkers.
    /// The map remains optional so standalone API callers without a compile
    /// snapshot retain the compiler's typed fallback configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oh_sdk_plugin_project_config: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeOhSdkConfig {
    pub api_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeOhSdkCheckPlugin {
    #[serde(rename = "osName")]
    pub os_name: String,
    pub tag: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
    pub path: PathBuf,
    pub function_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeOhSdkClassCheckPlugin {
    pub tag_name: String,
    pub path: PathBuf,
    pub class_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeOhSdkPluginRequest {
    pub session_id: u64,
    #[serde(default)]
    pub plugin_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<usize>,
    pub operation: String,
    #[serde(default)]
    pub args: Vec<Value>,
}

/// Session-local JS parser counters and V8 committed heap samples after node
/// preparation, plus committed heap before/after session compaction or release.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NativeOhSdkPluginStats {
    pub parses: u64,
    pub reparses: u64,
    pub batches: u64,
    pub peak_committed: u64,
    pub committed_before: u64,
    pub retained_committed: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NativeOhSdkPluginResponse {
    pub found: bool,
    pub result: bool,
    pub message: String,
    pub valid: bool,
    pub version: String,
    pub matched: bool,
    pub groups: Vec<String>,
    pub phase: String,
    pub error: String,
    pub check_result: bool,
    pub check_message: String,
    pub skipped: bool,
    pub stats: Option<NativeOhSdkPluginStats>,
}

pub type NativeOhSdkPluginCompletion =
    Box<dyn FnOnce(Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError>) + Send>;

/// JavaScript owns only the source-observed SDK CommonJS plugin ABI. tsrs
/// decides when callbacks participate in type diagnostics; Rust routes the
/// typed request without loading or interpreting plugin code.
pub trait NativeOhSdkPluginHost: Send + Sync + std::fmt::Debug {
    fn invoke_batch(
        &self,
        requests: Vec<NativeOhSdkPluginRequest>,
    ) -> Result<Vec<NativeOhSdkPluginResponse>, NativeCheckError>;

    /// Complete exactly once. Event-loop hosts enqueue without waiting; hosts
    /// with a synchronous transport may finish on the submitting thread.
    fn submit_batch(
        &self,
        requests: Vec<NativeOhSdkPluginRequest>,
        complete: NativeOhSdkPluginCompletion,
    ) {
        complete(self.invoke_batch(requests));
    }
}

impl NativeCompilerOptions {
    /// ets2bundle/compiler/tsconfig.json compiler options. Module resolution
    /// Node10 is passed verbatim; the native compiler owns its interpretation.
    pub fn arkts11(project_root: &Path) -> Self {
        Self {
            ets: Arc::default(),
            ets_loader_path: PathBuf::new(),
            compile_sdk_version: 0,
            target: 8, // core.ScriptTargetES2021
            module: 1, // ets_checker.ts::setCompilerOptions CommonJS
            module_resolution: 2,
            lib: vec!["lib.es2021.d.ts".into()],
            types: Vec::new(),
            type_roots: Vec::new(),
            paths: BTreeMap::new(),
            paths_base_path: project_root.to_path_buf(),
            base_url: project_root.to_path_buf(),
            no_emit: true,
            // ArkTS retains normal and strict pools together. Bound both
            // pools to four checkers so duplicated inference state stays
            // within the native service memory budget.
            checkers: std::thread::available_parallelism()
                .map_or(4, |count| count.get())
                .min(4),
            incremental: true,
            ts_build_info_file: PathBuf::new(),
            skip_paths_in_key_for_compilation_settings: true,
            skip_base_url_in_key_for_compilation_settings: true,
            allow_js: true,
            check_js: false,
            emit_node_modules_files: true,
            allow_synthetic_default_imports: true,
            es_module_interop: true,
            experimental_decorators: true,
            resolve_json_module: true,
            skip_lib_check: true,
            strict: false,
            strict_null_checks: false,
            always_strict: true,
            is_compile_js_har: false,
            module_root_path: PathBuf::new(),
            max_flow_depth: 2000.0,
            // ets2bundle compiler/main.js enables annotations at build startup.
            ets_annotations_enable: true,
            need_do_ark_ts_linter: true,
            is_compatible_version: false,
            ts_import_sendable_enable: false,
            ts_import_so_check: false,
            package_manager_type: "ohpm".into(),
            skip_tsc_oh_module_check: false,
            skip_ark_ts_static_blocks_check: false,
            compatible_sdk_version: None,
            compatible_sdk_version_stage: String::new(),
            skip_oh_modules_lint: false,
            enable_strict_check_oh_module: false,
            disable_strict_check_paths: Vec::new(),
            disable_sendable_check_rules: Vec::new(),
            mix_compile: false,
            strict_checker_only: false,
            oh_sdk_configs: Vec::new(),
            oh_system_modules: Vec::new(),
            oh_sdk_config_prefixes: Vec::new(),
            oh_fallback_module_roots: Vec::new(),
            oh_loader_module_root: PathBuf::new(),
            oh_project_path: PathBuf::new(),
            oh_external_api_paths: Vec::new(),
            oh_package_exports: BTreeMap::new(),
            oh_runtime_os: String::new(),
            oh_origin_compatible_sdk_version: String::new(),
            oh_project_root_path: PathBuf::new(),
            oh_module_path: PathBuf::new(),
            oh_all_module_paths: Vec::new(),
            oh_global_module_paths: Vec::new(),
            oh_ark_ui_declaration_dirs: Vec::new(),
            oh_request_permissions: Vec::new(),
            oh_syscap_intersection: Vec::new(),
            oh_syscap_union: Vec::new(),
            oh_device_types: Vec::new(),
            oh_card_entry_files: Vec::new(),
            oh_crossplatform: false,
            oh_ignore_crossplatform_check: false,
            oh_compile_mode: String::new(),
            oh_bundle_type: String::new(),
            oh_api_compatibility_check: String::new(),
            oh_sdk_check_plugins: Vec::new(),
            oh_sdk_class_check_plugins: Vec::new(),
            oh_sdk_plugin_project_config: None,
        }
    }

    pub fn uses_sdk_plugin_callbacks(&self) -> bool {
        !self.oh_sdk_check_plugins.is_empty() || !self.oh_sdk_class_check_plugins.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeDiagnostic {
    #[serde(default)]
    pub file_name: Option<PathBuf>,
    /// tsrs `DiagnosticResponse` uses signed source offsets, and diagnostics without a
    /// source file may retain the compiler's negative sentinel range.
    pub pos: i64,
    pub end: i64,
    pub code: i32,
    pub category: u8,
    #[serde(default)]
    pub source: String,
    pub text: String,
    pub start_position: Option<DiagnosticPosition>,
    pub end_position: Option<DiagnosticPosition>,
    #[serde(default)]
    pub message_chain: Vec<NativeDiagnostic>,
    #[serde(default)]
    pub related_information: Vec<NativeDiagnostic>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiagnosticPosition {
    pub line: u32,
    pub character: u32,
}

impl NativeDiagnostic {
    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            file_name: None,
            pos: 0,
            end: 0,
            code: -1,
            category: 0,
            source: String::new(),
            text: text.into(),
            start_position: None,
            end_position: None,
            message_chain: Vec::new(),
            related_information: Vec::new(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.category == 1 // diagnostics.CategoryError
    }

    pub fn render(&self) -> String {
        let prefix = if self.source.is_empty() {
            "TS"
        } else {
            &self.source
        };
        let location = self.file_name.as_ref().map_or_else(String::new, |path| {
            self.start_position.as_ref().map_or_else(
                || format!("{} ", path.display()),
                |position| {
                    format!(
                        "{}:{}:{} ",
                        path.display(),
                        position.line + 1,
                        position.character + 1
                    )
                },
            )
        });
        let mut message = format!("{location}{prefix}{} {}", self.code, self.text);
        for child in &self.message_chain {
            message.push_str("\n  ");
            message.push_str(&child.render());
        }
        for related in &self.related_information {
            message.push_str("\n  ");
            message.push_str(&related.render());
        }
        message
    }
}

/// One source-ordered build diagnostic batch. The native compiler reuses the
/// normal checker result produced for ArkTS strict-diagnostic comparison,
/// matching the upstream BuilderProgram cache lifecycle.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NativeArktsBuildDiagnostics {
    pub linter_diagnostics: Vec<NativeDiagnostic>,
    pub syntactic_diagnostics: Vec<NativeDiagnostic>,
    pub semantic_diagnostics: Vec<NativeDiagnostic>,
}

impl From<tsrs_api::embedded::ArktsBuildDiagnostics> for NativeArktsBuildDiagnostics {
    fn from(d: tsrs_api::embedded::ArktsBuildDiagnostics) -> Self {
        Self {
            linter_diagnostics: d.linter_diagnostics.into_iter().map(Into::into).collect(),
            syntactic_diagnostics: d
                .syntactic_diagnostics
                .into_iter()
                .map(Into::into)
                .collect(),
            semantic_diagnostics: d.semantic_diagnostics.into_iter().map(Into::into).collect(),
        }
    }
}

/// Filesystem invalidation passed to an incremental native program update.
///
/// `rollup-plugin-watch-change.ts::watchChange` preserves update separately
/// from create/delete. The tsrs API exposes the same three-way distinction, so
/// the Rust watch owner must not collapse it to a bare changed-path list.
#[derive(Debug, Clone, Default)]
pub struct NativeProgramFileChanges {
    pub changed: Vec<PathBuf>,
    pub created: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

/// Build-owned preprocessing is injected without a dependency on transforms.
/// SourceSnapshot calls this once per read implementation-file version.
pub trait NativeSourcePreprocessor: Send + Sync {
    fn preprocess(&self, path: &Path, source: Arc<str>) -> NativeProcessedSource;
}

#[derive(Debug, Clone, Default)]
pub struct NativeProcessedSource {
    pub text: Arc<str>,
    pub legacy_extend_names: BTreeSet<String>,
    pub new_extend_names: BTreeSet<String>,
}

/// Owns the compiler and the current immutable input inventory. Program/type
/// leases own their snapshots independently and may safely outlive this session.
pub struct NativeTypeSession {
    compiler: Compiler,
    base: Arc<dyn FS>,
    sources: BTreeMap<PathBuf, Arc<str>>,
    deleted: BTreeSet<PathBuf>,
    sdk_plugin_host: Option<Arc<plugins::PluginHost>>,
    sdk_plugin_executor: Option<Arc<dyn tsrs_core::OhSdkPluginExecutor>>,
    preprocessor: Option<Arc<dyn NativeSourcePreprocessor>>,
    snapshot: Option<Arc<SourceSnapshot>>,
}

impl NativeTypeSession {
    pub fn start(
        project_root: &Path,
        sources: BTreeMap<PathBuf, Arc<str>>,
        sdk_plugin_host: Option<Arc<dyn NativeOhSdkPluginHost>>,
    ) -> Result<Self, NativeCheckError> {
        let base: Arc<dyn FS> = Arc::new(bundled::wrap_fs(osvfs::fs()));
        let compiler = Compiler::new(tsrs_api::SessionOptions::new(
            options::CompilerPaths::one(project_root)?,
            bundled::lib_path(),
            Arc::clone(&base),
            false,
        ))?;
        let sdk_plugin_host = sdk_plugin_host.map(|host| Arc::new(plugins::PluginHost::new(host)));
        let sdk_plugin_executor = sdk_plugin_host.as_ref().map(|host| {
            Arc::new(tsrs_api::embedded::SdkPluginExecutor::new(host.clone()))
                as Arc<dyn tsrs_core::OhSdkPluginExecutor>
        });
        Ok(Self {
            compiler,
            base,
            sources,
            deleted: BTreeSet::new(),
            sdk_plugin_host,
            sdk_plugin_executor,
            preprocessor: None,
            snapshot: None,
        })
    }

    pub fn with_source_preprocessor(
        mut self,
        preprocessor: Arc<dyn NativeSourcePreprocessor>,
    ) -> Self {
        assert!(
            self.snapshot.is_none(),
            "set preprocessing before creating a program"
        );
        self.preprocessor = Some(preprocessor);
        self
    }

    /// Initialized source versions from the current input snapshot. Callers
    /// select the current Program graph before consuming diagnostic names/facts.
    pub fn read_sources(&self) -> BTreeMap<PathBuf, NativeProcessedSource> {
        self.snapshot
            .as_ref()
            .map(|snapshot| snapshot.read_sources())
            .unwrap_or_default()
    }

    pub fn create_program(
        &mut self,
        roots: &[PathBuf],
        options: &NativeCompilerOptions,
    ) -> Result<NativeProgram, NativeCheckError> {
        let fs = Arc::new(SourceSnapshot::new(
            Arc::clone(&self.base),
            &self.sources,
            &self.deleted,
            self.preprocessor.clone(),
            None,
            &BTreeSet::new(),
        )?);
        let program = self.compiler.create_program(
            &options::CompilerPaths::many(roots)?,
            options.to_tsrs(self.sdk_plugin_executor.clone())?,
            fs.clone(),
        )?;
        self.snapshot = Some(fs);
        Ok(program)
    }
    /// Reconfigure the synthetic project in a new snapshot. Preserve the
    /// previous snapshot until its replacement has been created successfully.
    pub fn update_program(
        &mut self,
        previous: NativeProgram,
        roots: &[PathBuf],
        options: &NativeCompilerOptions,
        changed: BTreeMap<PathBuf, Arc<str>>,
        deleted: &[PathBuf],
    ) -> Result<NativeProgram, NativeCheckError> {
        let mut changes = NativeProgramFileChanges::default();
        for path in changed.keys() {
            if self.sources.contains_key(path) {
                changes.changed.push(path.clone());
            } else {
                changes.created.push(path.clone());
            }
        }
        changes.deleted.extend_from_slice(deleted);
        let mut sources = self.sources.clone();
        sources.extend(changed);
        for path in deleted {
            sources.remove(path);
        }
        self.update_program_snapshot(previous, roots, options, sources, &changes)
    }

    /// Update a retained program from the next immutable source snapshot.
    ///
    /// The source diff protects callers that rebuild their input inventory,
    /// while `observed` carries watcher events for compiler-owned disk files
    /// that are intentionally absent from the callback source map.
    pub fn update_program_snapshot(
        &mut self,
        previous: NativeProgram,
        roots: &[PathBuf],
        options: &NativeCompilerOptions,
        sources: BTreeMap<PathBuf, Arc<str>>,
        observed: &NativeProgramFileChanges,
    ) -> Result<NativeProgram, NativeCheckError> {
        let mut changed = observed.changed.iter().cloned().collect::<BTreeSet<_>>();
        let mut created = observed.created.iter().cloned().collect::<BTreeSet<_>>();
        let mut deleted = observed.deleted.iter().cloned().collect::<BTreeSet<_>>();

        for (path, content) in &sources {
            match self.sources.get(path) {
                Some(previous) if previous.as_ref() != content.as_ref() => {
                    deleted.remove(path);
                    changed.insert(path.clone());
                }
                None => {
                    deleted.remove(path);
                    created.insert(path.clone());
                }
                _ => {
                    deleted.remove(path);
                }
            }
        }
        for path in self.sources.keys() {
            if !sources.contains_key(path) {
                deleted.insert(path.clone());
            }
        }

        // A final filesystem state has one meaning even when an editor emits
        // several events for the same path in a single debounced batch.
        for path in &deleted {
            changed.remove(path);
            created.remove(path);
        }
        for path in &created {
            changed.remove(path);
        }

        let mut tombstones = self.deleted.clone();
        for path in changed.iter().chain(created.iter()) {
            tombstones.remove(path);
        }
        tombstones.extend(deleted.iter().cloned());
        let fs = Arc::new(SourceSnapshot::new(
            Arc::clone(&self.base),
            &sources,
            &tombstones,
            self.preprocessor.clone(),
            self.snapshot.as_deref(),
            &changed
                .iter()
                .chain(&created)
                .chain(&deleted)
                .cloned()
                .collect(),
        )?);
        let notifications = tsrs_api::embedded::FileChanges {
            changed: options::CompilerPaths::many(&changed.into_iter().collect::<Vec<_>>())?,
            created: options::CompilerPaths::many(&created.into_iter().collect::<Vec<_>>())?,
            deleted: options::CompilerPaths::many(&deleted.into_iter().collect::<Vec<_>>())?,
        };
        let next = self.compiler.update_program(
            &previous,
            &options::CompilerPaths::many(roots)?,
            options.to_tsrs(self.sdk_plugin_executor.clone())?,
            fs.clone(),
            &notifications,
        )?;
        self.sources = sources;
        self.deleted = tombstones;
        self.snapshot = Some(fs);
        Ok(next)
    }

    pub fn non_nullable_type(
        &mut self,
        program: &NativeProgram,
        ty: &NativeType,
    ) -> Result<NativeType, NativeCheckError> {
        Ok(program.non_nullable_type(ty)?)
    }
    pub fn type_symbol(
        &mut self,
        program: &NativeProgram,
        ty: &NativeType,
    ) -> Result<Option<NativeSymbol>, NativeCheckError> {
        Ok(program.type_symbol(ty)?)
    }
    pub fn arkts_build_diagnostics(
        &mut self,
        program: &NativeProgram,
    ) -> Result<NativeArktsBuildDiagnostics, NativeCheckError> {
        Ok(program.arkts_build_diagnostics()?.into())
    }
    pub fn arkts_normal_diagnostics(
        &mut self,
        program: &NativeProgram,
    ) -> Result<NativeArktsNormalDiagnostics, NativeCheckError> {
        Ok(program.arkts_normal_diagnostics()?)
    }
    pub fn arkts_strict_diagnostics(
        &mut self,
        program: &NativeProgram,
        normal: NativeArktsNormalDiagnostics,
    ) -> Result<NativeArktsBuildDiagnostics, NativeCheckError> {
        // Normal-pool diagnostics and transform facts are owned at this boundary.
        // Strict may still query SDK plugins; keep their session and source snapshots.
        if let Some(host) = &self.sdk_plugin_host {
            host.compact_session();
        }
        Ok(program.arkts_strict_diagnostics(normal)?.into())
    }
    pub fn release_arkts_build_checkers(
        &mut self,
        program: &NativeProgram,
    ) -> Result<(), NativeCheckError> {
        Ok(program.release_arkts_build_checkers()?)
    }
    pub fn diagnostics(
        &mut self,
        program: &NativeProgram,
    ) -> Result<Vec<NativeDiagnostic>, NativeCheckError> {
        let mut diagnostics = self.syntactic_diagnostics(program)?;
        diagnostics.extend(
            program
                .semantic_diagnostics()?
                .into_iter()
                .map(NativeDiagnostic::from),
        );
        Ok(diagnostics)
    }
    pub fn arkts_linter_diagnostics(
        &mut self,
        program: &NativeProgram,
    ) -> Result<Vec<NativeDiagnostic>, NativeCheckError> {
        Ok(program
            .arkts_linter_diagnostics()?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    pub fn syntactic_diagnostics(
        &mut self,
        program: &NativeProgram,
    ) -> Result<Vec<NativeDiagnostic>, NativeCheckError> {
        Ok(program
            .syntactic_diagnostics()?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    pub fn declaration_diagnostics(
        &mut self,
        program: &NativeProgram,
        files: &[PathBuf],
    ) -> Result<Vec<NativeDiagnostic>, NativeCheckError> {
        Ok(program
            .declaration_diagnostics(&options::CompilerPaths::many(files)?)?
            .into_iter()
            .map(Into::into)
            .collect())
    }
    pub fn source_graph(
        &mut self,
        program: &NativeProgram,
    ) -> Result<NativeProgramSourceGraph, NativeCheckError> {
        Ok(program.source_graph()?)
    }
    pub fn annotation_transform_infos(
        &mut self,
        program: &NativeProgram,
        files: &[PathBuf],
    ) -> Result<Vec<NativeAnnotationTransformInfo>, NativeCheckError> {
        Ok(program.annotation_transform_infos(&options::CompilerPaths::many(files)?)?)
    }
    pub fn arkts_transform_type_facts(
        &mut self,
        program: &NativeProgram,
        queries: &[NativeArktsTransformTypeQuery],
        sdk_api_use_files: &[PathBuf],
    ) -> Result<Vec<NativeArktsTransformTypeFacts>, NativeCheckError> {
        Ok(program.arkts_transform_type_facts(
            queries,
            &options::CompilerPaths::many(sdk_api_use_files)?,
        )?)
    }
    pub fn types_at_positions(
        &mut self,
        program: &NativeProgram,
        file: &Path,
        positions: &[u32],
    ) -> Result<Vec<Option<NativeType>>, NativeCheckError> {
        Ok(program
            .types_at_positions(&options::CompilerPaths::one(file)?, positions)?
            .into_iter()
            .map(Some)
            .collect())
    }
}

impl From<tsrs_api::embedded::Diagnostic> for NativeDiagnostic {
    fn from(d: tsrs_api::embedded::Diagnostic) -> Self {
        Self {
            file_name: d.file_name,
            pos: d.pos,
            end: d.end,
            code: d.code,
            category: d.category,
            source: d.source,
            text: d.text,
            start_position: d.start_position.map(Into::into),
            end_position: d.end_position.map(Into::into),
            message_chain: d.message_chain.into_iter().map(Into::into).collect(),
            related_information: d.related_information.into_iter().map(Into::into).collect(),
        }
    }
}
impl From<tsrs_api::embedded::DiagnosticPosition> for DiagnosticPosition {
    fn from(p: tsrs_api::embedded::DiagnosticPosition) -> Self {
        Self {
            line: p.line,
            character: p.character,
        }
    }
}
