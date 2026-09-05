//! # oxc_checker
//!
//! Experimental **eager** TypeScript type checker built on **isolated
//! declarations**. See `DESIGN.md` for the architecture.
//!
//! Pipeline: load all files in parallel waves (parse + forced
//! `IsolatedDeclarations` surface extraction + module resolution) → link the
//! surfaces into a frozen, `Send + Sync` [`link::ProgramEnv`] → check every
//! file in parallel against it.

mod check;
mod diagnostics;
mod ir;
mod link;
mod loader;
mod lower;
mod surface;
mod tsconfig;

use std::path::{Path, PathBuf};

use oxc_diagnostics::OxcDiagnostic;

pub use crate::{
    ir::{FileId, SymbolId, TypeId},
    link::ProgramEnv,
    tsconfig::TsConfig,
};

/// Diagnostics for one file.
#[derive(Debug)]
pub struct FileResult {
    /// Absolute path.
    pub path: PathBuf,
    /// Source text (for rendering diagnostics).
    pub source_text: String,
    /// All diagnostics, ordered by source position.
    pub diagnostics: Vec<OxcDiagnostic>,
}

/// Result of checking a project.
#[derive(Debug)]
pub struct CheckResult {
    /// Per-file results, in deterministic (discovery) order.
    pub files: Vec<FileResult>,
}

impl CheckResult {
    /// Total number of diagnostics.
    pub fn error_count(&self) -> usize {
        self.files.iter().map(|f| f.diagnostics.len()).sum()
    }
}

/// Typed project inputs for an embedded checker invocation.
#[derive(Debug)]
pub struct CheckOptions {
    /// Project directory used to resolve relative roots and module specifiers.
    pub project_root: PathBuf,
    /// Root implementation files, matching TypeScript's `rootNames` input.
    pub root_files: Vec<PathBuf>,
    /// Optional tsconfig used only by module resolution.
    pub tsconfig_path: Option<PathBuf>,
    /// Additional package roots such as OpenHarmony `oh_modules` directories.
    pub module_paths: Vec<PathBuf>,
    /// Resolver aliases derived from the owning build system's package graph.
    pub aliases: Vec<(String, PathBuf)>,
    /// Whether `null` and `undefined` participate in strict relations.
    pub strict_null_checks: bool,
    /// Skip diagnostics originating in declaration files, matching
    /// TypeScript's `skipLibCheck` compiler option.
    pub skip_lib_check: bool,
    /// Report diagnostics produced by the isolated-declaration surface pass.
    pub report_isolated_declaration_diagnostics: bool,
}

/// Check explicit roots without invoking a command-line frontend.
///
/// # Errors
/// When no root files were provided.
pub fn check(options: CheckOptions) -> Result<CheckResult, String> {
    if options.root_files.is_empty() {
        return Err(format!(
            "No TypeScript or ArkTS files found under {}",
            options.project_root.display()
        ));
    }
    let skip_lib_check = options.skip_lib_check;
    let roots = options
        .root_files
        .into_iter()
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                options.project_root.join(path)
            }
        })
        .collect();
    let resolver = make_resolver(
        options.tsconfig_path,
        &options.module_paths,
        &options.aliases,
    );
    let loaded = loader::load(
        roots,
        &resolver,
        options.report_isolated_declaration_diagnostics,
    );
    let env = link::link(loaded, options.strict_null_checks);
    let mut per_file = check::check_program(&env);

    let files = env
        .files
        .into_iter()
        .zip(per_file.iter_mut())
        .map(|(file, checked)| {
            let mut diagnostics = file.diagnostics;
            diagnostics.append(checked);
            if skip_lib_check && is_declaration_file(&file.path) {
                diagnostics.clear();
            }
            diagnostics.sort_by_key(|d| {
                d.labels
                    .first()
                    .map_or(0, oxc_diagnostics::LabeledSpan::offset)
            });
            FileResult {
                path: file.path,
                source_text: file.source_text,
                diagnostics,
            }
        })
        .collect();

    Ok(CheckResult { files })
}

/// Check a project rooted at a directory or described by a `tsconfig.json`.
///
/// # Errors
/// When the tsconfig cannot be loaded or no TypeScript files are found.
pub fn check_project(path: &Path) -> Result<CheckResult, String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string());
    let path = path?;
    let (dir, tsconfig_path) = if path.is_dir() {
        let tsconfig = path.join("tsconfig.json");
        (path, tsconfig.is_file().then_some(tsconfig))
    } else {
        let dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        (dir, Some(path))
    };

    let config = match &tsconfig_path {
        Some(p) => TsConfig::load(p)?,
        None => TsConfig::default(),
    };

    let roots = config.root_files(&dir);
    check(CheckOptions {
        project_root: dir,
        root_files: roots,
        tsconfig_path,
        module_paths: Vec::new(),
        aliases: Vec::new(),
        strict_null_checks: config.strict_null_checks(),
        skip_lib_check: config.skip_lib_check(),
        report_isolated_declaration_diagnostics: config.isolated_declarations(),
    })
}

fn is_declaration_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    name.ends_with(".d.ts")
        || name.ends_with(".d.ets")
        || name.ends_with(".d.mts")
        || name.ends_with(".d.cts")
}

fn make_resolver(
    tsconfig: Option<PathBuf>,
    module_paths: &[PathBuf],
    aliases: &[(String, PathBuf)],
) -> oxc_resolver::Resolver {
    use oxc_resolver::{
        ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
    };
    Resolver::new(ResolveOptions {
        alias: aliases
            .iter()
            .map(|(specifier, target)| {
                (
                    specifier.clone(),
                    vec![oxc_resolver::AliasValue::Path(
                        target.to_string_lossy().into_owned(),
                    )],
                )
            })
            .collect(),
        extensions: [
            ".ets", ".d.ets", ".ts", ".tsx", ".d.ts", ".mts", ".cts", ".d.mts", ".d.cts", ".js",
            ".mjs", ".cjs", ".json",
        ]
        .map(String::from)
        .into(),
        extension_alias: vec![
            (
                ".js".into(),
                vec![".ts".into(), ".tsx".into(), ".d.ts".into(), ".js".into()],
            ),
            (
                ".mjs".into(),
                vec![".mts".into(), ".d.mts".into(), ".mjs".into()],
            ),
            (
                ".cjs".into(),
                vec![".cts".into(), ".d.cts".into(), ".cjs".into()],
            ),
            (
                ".ts".into(),
                vec![".ts".into(), ".d.ts".into(), ".tsx".into()],
            ),
            (".mts".into(), vec![".mts".into(), ".d.mts".into()]),
            (".cts".into(), vec![".cts".into(), ".d.cts".into()]),
            (".ets".into(), vec![".ets".into(), ".d.ets".into()]),
        ],
        condition_names: vec![
            "types".into(),
            "import".into(),
            "require".into(),
            "node".into(),
        ],
        main_fields: vec!["types".into(), "module".into(), "main".into()],
        modules: std::iter::once("node_modules".to_string())
            .chain(
                module_paths
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned()),
            )
            .collect(),
        // `node:*` and bare builtins resolve as builtin errors, which the
        // loader treats as external (`any`) modules.
        builtin_modules: true,
        tsconfig: tsconfig.map(|config_file| {
            TsconfigDiscovery::Manual(TsconfigOptions {
                config_file,
                references: TsconfigReferences::Auto,
            })
        }),
        ..ResolveOptions::default()
    })
}
