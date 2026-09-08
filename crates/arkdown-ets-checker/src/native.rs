//! Native TypeScript API client. Protocol owner: the packaged ArkTS TSGO,
//! tsc/internal/api/{proto,session}.go and ipc/protocol_jsonrpc.go.
//! This is an internal compiler service, never an arkdown command-line entry.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

#[derive(Debug, thiserror::Error)]
pub enum NativeCheckError {
    #[error("ARKDOWN_TYPE_SERVICE: {0}")]
    Protocol(String),
    #[error("ARKDOWN_TYPE_SERVICE_IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("ARKDOWN_TYPE_SERVICE_JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Actual Go API compiler options (enum numbers, not tsconfig strings).
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeDiagnostic {
    #[serde(default)]
    pub file_name: Option<PathBuf>,
    pub pos: u32,
    pub end: u32,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NativeProgram {
    snapshot: u64,
    project: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeType {
    pub id: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeSymbol {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAnnotationTransformInfo {
    pub file_name: PathBuf,
    pub declarations: Vec<NativeAnnotationTransformNode>,
    pub uses: Vec<NativeAnnotationTransformNode>,
    pub imports: Vec<NativeAnnotationTransformImport>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeAnnotationTransformNode {
    pub pos: usize,
    pub end: usize,
    #[serde(default, rename = "runtimeRetained")]
    pub runtime_retained: bool,
    pub info: NativeAnnotationInfo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAnnotationTransformImport {
    pub pos: usize,
    pub end: usize,
    pub disposition: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAnnotationInfo {
    pub source_retention: bool,
    pub properties: Vec<NativeAnnotationProperty>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAnnotationProperty {
    pub name: String,
    pub name_text: String,
    pub initializer: Option<NativeAnnotationConstant>,
    pub argument: Option<NativeAnnotationConstant>,
    pub array_depth: usize,
    pub enum_first_value: Option<NativeAnnotationConstant>,
    pub type_text: String,
    pub element_type_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NativeAnnotationConstant {
    pub kind: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub items: Vec<NativeAnnotationConstant>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeArktsTransformTypeFacts {
    pub file_name: PathBuf,
    pub properties: Vec<NativeArktsPropertyTypeFacts>,
    pub builder_accesses: Vec<NativeArktsBuilderReceiverTypeFacts>,
    pub member_accesses: Vec<NativeArktsExpressionTypeFacts>,
    pub sdk_api_uses: Vec<NativeSdkApiUse>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeSdkApiUse {
    pub api_module: String,
    pub function: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeArktsPropertyTypeFacts {
    pub name_pos: usize,
    pub name_end: usize,
    pub r#type: Option<NativeResolvedTypeIdentity>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeArktsBuilderReceiverTypeFacts {
    pub pos: usize,
    pub end: usize,
    pub receiver_type: NativeResolvedTypeIdentity,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeArktsExpressionTypeFacts {
    pub pos: usize,
    pub end: usize,
    pub r#type: NativeResolvedTypeIdentity,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeResolvedTypeIdentity {
    pub is_nullable: bool,
    pub is_enum: bool,
    pub is_basic: bool,
    pub symbol_name: String,
    pub types: Vec<NativeResolvedTypeIdentity>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeProgramSourceGraph {
    pub files: Vec<NativeProgramSourceGraphFile>,
    pub type_reference_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeProgramSourceGraphFile {
    pub file_name: PathBuf,
    pub dependencies: Vec<PathBuf>,
}

#[derive(Deserialize)]
struct CreatedProgram {
    snapshot: u64,
    project: Project,
}

#[derive(Deserialize)]
struct Project {
    id: String,
}

/// Owns the child and every request. Go retains AST/checker memory; Rust
/// retains source snapshots and owned results. No shared-write hook collector.
pub struct NativeTypeSession {
    child: Child,
    input: Option<ChildStdin>,
    messages: mpsc::Receiver<Result<Value, NativeCheckError>>,
    reader: Option<JoinHandle<()>>,
    stderr: Option<JoinHandle<Vec<u8>>>,
    next_id: u64,
    sources: BTreeMap<PathBuf, Arc<str>>,
    deleted: BTreeSet<PathBuf>,
}

impl NativeTypeSession {
    pub fn executable() -> Result<PathBuf, NativeCheckError> {
        Self::resolve_executable(None)
    }

    pub fn resolve_executable(
        packaged_executable: Option<&Path>,
    ) -> Result<PathBuf, NativeCheckError> {
        let configured = std::env::var_os("ARKDOWN_TYPESCRIPT_PATH")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let path = configured
            .as_deref()
            .or(packaged_executable)
            .ok_or_else(|| {
                NativeCheckError::Protocol(
                    "native ArkTS TypeScript service is missing from the platform package; ARKDOWN_TYPESCRIPT_PATH may be used only as a development override".into(),
                )
            })?;
        if !path.is_file() {
            return Err(NativeCheckError::Protocol(format!(
                "native ArkTS TypeScript service is not a file: {}",
                path.display()
            )));
        }
        Ok(path.canonicalize()?)
    }

    pub fn start(
        executable: &Path,
        project_root: &Path,
        sources: BTreeMap<PathBuf, Arc<str>>,
    ) -> Result<Self, NativeCheckError> {
        let node_executable = std::env::current_exe()
            .ok()
            .filter(|path| {
                path.file_stem()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("node"))
            })
            .unwrap_or_else(|| PathBuf::from("node"));
        let mut child = Command::new(executable)
            .args(["--api", "--async", "--cwd"])
            .arg(project_root)
            .args(["--callbacks", "readFile,fileExists"])
            .arg("--runExternalCode")
            .arg("--nodeExecutable")
            .arg(node_executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let input = child.stdin.take();
        let mut wire = NativeMessageReader {
            output: BufReader::new(child.stdout.take().expect("piped stdout")),
        };
        let (sender, messages) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            loop {
                let message = wire.read();
                let failed = message.is_err();
                if sender.send(message).is_err() || failed {
                    break;
                }
            }
        });
        let mut errors = child.stderr.take().expect("piped stderr");
        let stderr = std::thread::spawn(move || {
            let mut tail = Vec::new();
            let mut buffer = [0; 4096];
            while let Ok(count) = errors.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                tail.extend_from_slice(&buffer[..count]);
                if tail.len() > 16384 {
                    tail.drain(..tail.len() - 16384);
                }
            }
            tail
        });
        let mut session = Self {
            child,
            input,
            messages,
            reader: Some(reader),
            stderr: Some(stderr),
            next_id: 0,
            sources,
            deleted: BTreeSet::new(),
        };
        let _: Value = session.request("initialize", json!({}))?;
        Ok(session)
    }

    pub fn create_program(
        &mut self,
        roots: &[PathBuf],
        options: &NativeCompilerOptions,
    ) -> Result<NativeProgram, NativeCheckError> {
        let created: CreatedProgram = self.request(
            "createProgram",
            json!({
                "rootFiles": roots,
                "createProgramOptions": { "compilerOptions": options }
            }),
        )?;
        Ok(NativeProgram {
            snapshot: created.snapshot,
            project: created.project.id,
        })
    }

    /// api/session.go::handleCreateProgram explicitly accepts oldProgram and
    /// fileChanges. Preserve the previous snapshot until the new one exists.
    pub fn update_program(
        &mut self,
        previous: NativeProgram,
        roots: &[PathBuf],
        options: &NativeCompilerOptions,
        changed: BTreeMap<PathBuf, Arc<str>>,
        deleted: &[PathBuf],
    ) -> Result<NativeProgram, NativeCheckError> {
        let changed_files: Vec<_> = changed.keys().cloned().collect();
        for path in &changed_files {
            self.deleted.remove(path);
        }
        self.sources.extend(changed);
        for path in deleted {
            self.sources.remove(path);
            self.deleted.insert(path.clone());
        }
        let created: CreatedProgram = self.request(
            "createProgram",
            json!({
                "rootFiles": roots,
                "createProgramOptions": { "compilerOptions": options },
                "oldProgram": previous,
                "fileChanges": { "changed": changed_files, "deleted": deleted }
            }),
        )?;
        self.release(previous)?;
        Ok(NativeProgram {
            snapshot: created.snapshot,
            project: created.project.id,
        })
    }

    pub fn non_nullable_type(
        &mut self,
        program: &NativeProgram,
        ty: &NativeType,
    ) -> Result<NativeType, NativeCheckError> {
        self.request(
            "getNonNullableType",
            json!({
                "snapshot": program.snapshot, "project": program.project, "objectId": ty.id
            }),
        )
    }

    pub fn type_symbol(
        &mut self,
        program: &NativeProgram,
        ty: &NativeType,
    ) -> Result<Option<NativeSymbol>, NativeCheckError> {
        self.request(
            "getSymbolOfType",
            json!({
                "snapshot": program.snapshot, "project": program.project, "objectId": ty.id
            }),
        )
    }

    /// Mirrors ets_checker.ts::processBuildHap: syntax then semantic diagnostics.
    /// No per-root filtering: diagnostics in imported implementation files matter.
    pub fn diagnostics(
        &mut self,
        program: &NativeProgram,
    ) -> Result<Vec<NativeDiagnostic>, NativeCheckError> {
        let mut diagnostics = self.syntactic_diagnostics(program)?;
        let result: Option<Vec<NativeDiagnostic>> =
            self.request("getSemanticDiagnostics", serde_json::to_value(program)?)?;
        diagnostics.extend(result.unwrap_or_default());
        Ok(diagnostics)
    }

    pub fn arkts_linter_diagnostics(
        &mut self,
        program: &NativeProgram,
    ) -> Result<Vec<NativeDiagnostic>, NativeCheckError> {
        let result: Option<Vec<NativeDiagnostic>> =
            self.request("getArkTSLinterDiagnostics", serde_json::to_value(program)?)?;
        Ok(result.unwrap_or_default())
    }

    pub fn source_graph(
        &mut self,
        program: &NativeProgram,
    ) -> Result<NativeProgramSourceGraph, NativeCheckError> {
        self.request("getProgramSourceGraph", serde_json::to_value(program)?)
    }

    /// Checker-owned facts for OH `ohApi.ts::transformAnnotation`, batched for
    /// all selected transform inputs so a module build performs one IPC call.
    pub fn annotation_transform_infos(
        &mut self,
        program: &NativeProgram,
        files: &[PathBuf],
    ) -> Result<Vec<NativeAnnotationTransformInfo>, NativeCheckError> {
        self.request(
            "getAnnotationTransformInfos",
            json!({
                "snapshot": program.snapshot,
                "project": program.project,
                "files": files,
            }),
        )
    }

    /// Immutable type identities used by ArkTS 1.1 OXC transforms. The Go
    /// service performs all queries in one checker lease and returns no handles.
    pub fn arkts_transform_type_facts(
        &mut self,
        program: &NativeProgram,
        files: &[PathBuf],
    ) -> Result<Vec<NativeArktsTransformTypeFacts>, NativeCheckError> {
        self.request(
            "getArkTSTransformTypeFacts",
            json!({
                "snapshot": program.snapshot,
                "project": program.project,
                "files": files,
            }),
        )
    }

    /// `ets_checker.ts::printDeclarationDiagnostics` requests declaration
    /// diagnostics one eligible implementation source at a time for HAR/HSP.
    pub fn declaration_diagnostics(
        &mut self,
        program: &NativeProgram,
        files: &[PathBuf],
    ) -> Result<Vec<NativeDiagnostic>, NativeCheckError> {
        let result: Option<Vec<NativeDiagnostic>> = self.request(
            "getDeclarationDiagnostics",
            json!({
                "snapshot": program.snapshot,
                "project": program.project,
                "files": files,
            }),
        )?;
        Ok(result.unwrap_or_default())
    }

    /// Native api/session.go::handleGetSyntacticDiagnostics. Keep syntax
    /// conformance independent of missing SDK symbols or semantic diagnostics;
    /// the build service still requests both phases through `diagnostics`.
    pub fn syntactic_diagnostics(
        &mut self,
        program: &NativeProgram,
    ) -> Result<Vec<NativeDiagnostic>, NativeCheckError> {
        let result: Option<Vec<NativeDiagnostic>> =
            self.request("getSyntacticDiagnostics", serde_json::to_value(program)?)?;
        Ok(result.unwrap_or_default())
    }

    /// API positions are UTF-16, explicitly unlike OXC spans. The caller must
    /// convert against the same source snapshot before submitting a batch.
    pub fn types_at_positions(
        &mut self,
        program: &NativeProgram,
        file: &Path,
        positions: &[u32],
    ) -> Result<Vec<Option<NativeType>>, NativeCheckError> {
        self.request(
            "getTypesAtPositions",
            json!({
                "snapshot": program.snapshot, "project": program.project,
                "file": file, "positions": positions
            }),
        )
    }

    pub fn release(&mut self, program: NativeProgram) -> Result<(), NativeCheckError> {
        let _: Value = self.request("release", json!({ "snapshot": program.snapshot }))?;
        Ok(())
    }

    fn request<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<T, NativeCheckError> {
        self.next_id += 1;
        let id = self.next_id;
        let deadline = Instant::now() + Duration::from_secs(120);
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        loop {
            let response = match self
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(Ok(response)) => response,
                failure => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    let tail = self
                        .stderr
                        .take()
                        .and_then(|thread| thread.join().ok())
                        .unwrap_or_default();
                    return Err(NativeCheckError::Protocol(format!(
                        "{method}: {failure:?}; compiler stderr: {}",
                        String::from_utf8_lossy(&tail)
                    )));
                }
            };
            if let Some(method) = response.get("method").and_then(Value::as_str) {
                let path = response
                    .get("params")
                    .and_then(Value::as_str)
                    .map(Path::new);
                let source = path.and_then(|path| self.sources.get(path));
                let deleted = path.is_some_and(|path| self.deleted.contains(path));
                // callbackfs.go: null delegates to the compiler's bundled/disk FS;
                // {content: null} instead means a definitive missing file.
                let result = match method {
                    "readFile" if deleted => json!({"content": null}),
                    "fileExists" if deleted => Value::Bool(false),
                    "readFile" => source.map_or(Value::Null, |text| json!({"content": &**text})),
                    "fileExists" => source.map_or(Value::Null, |_| Value::Bool(true)),
                    _ => {
                        return Err(NativeCheckError::Protocol(format!(
                            "unexpected callback {method}"
                        )));
                    }
                };
                self.write(&json!({"jsonrpc": "2.0", "id": response["id"], "result": result}))?;
                continue;
            }
            if response.get("id").and_then(Value::as_u64) != Some(id) {
                return Err(NativeCheckError::Protocol("mismatched response id".into()));
            }
            if let Some(error) = response.get("error") {
                return Err(NativeCheckError::Protocol(format!("{method}: {error}")));
            }
            let result = response
                .get("result")
                .ok_or_else(|| NativeCheckError::Protocol("missing response result".into()))?;
            return Ok(serde_json::from_value(result.clone())?);
        }
    }

    fn write(&mut self, message: &Value) -> Result<(), NativeCheckError> {
        let bytes = serde_json::to_vec(message)?;
        let input = self
            .input
            .as_mut()
            .ok_or_else(|| NativeCheckError::Protocol("closed session".into()))?;
        write!(input, "Content-Length: {}\r\n\r\n", bytes.len())?;
        input.write_all(&bytes)?;
        input.flush()?;
        Ok(())
    }
}

struct NativeMessageReader {
    output: BufReader<ChildStdout>,
}

impl NativeMessageReader {
    fn read(&mut self) -> Result<Value, NativeCheckError> {
        let mut length = None;
        let mut header_bytes = 0;
        loop {
            let mut line = String::new();
            let read = self.output.read_line(&mut line)?;
            if read == 0 {
                return Err(NativeCheckError::Protocol(
                    "compiler closed the API stream before responding".into(),
                ));
            }
            header_bytes += read;
            if header_bytes > 8192 {
                return Err(NativeCheckError::Protocol("oversized API header".into()));
            }
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                length =
                    Some(value.trim().parse::<usize>().map_err(|_| {
                        NativeCheckError::Protocol("invalid Content-Length".into())
                    })?);
            }
        }
        let length = length
            .filter(|length| *length <= 64 * 1024 * 1024)
            .ok_or_else(|| {
                NativeCheckError::Protocol("missing or oversized Content-Length".into())
            })?;
        let mut bytes = vec![0; length];
        self.output.read_exact(&mut bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

impl Drop for NativeTypeSession {
    fn drop(&mut self) {
        drop(self.input.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(stderr) = self.stderr.take() {
            let _ = stderr.join();
        }
    }
}
