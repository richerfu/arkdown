//! Rust-owned compiler for user-authored JavaScript and TypeScript plugins.
//!
//! OXC strips TypeScript and lowers syntax while Rolldown emits one ES2016
//! CommonJS module. User imports remain external so Node retains ownership of
//! package loading and executes the generated module in the caller's V8
//! isolate. No build configuration or task behavior is interpreted here.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{
    AttachDebugInfo, BundlerTransformOptions, DecoratorOptions, Either, EsModuleFlag,
    ExperimentalOptions, IsExternal, ModuleType, Output, OutputExports, Platform,
    PreserveEntrySignatures, SourceMapType, TreeshakeOptions, TypeScriptOptions,
};
use rustc_hash::FxHashMap;
use thiserror::Error;

use arkdown_cache::CacheFingerprint;
use arkdown_path::ArtifactPathSerializer;

use crate::diagnostics::RolldownBuildDiagnostics;

/// Compiles one plugin module without executing JavaScript.
///
/// Imports stay external so Node retains its module cache, extension handlers,
/// and `require` resolution behavior. Rolldown owns module syntax lowering and
/// CommonJS code generation.
#[derive(Debug, Clone)]
pub struct PluginModuleCompiler {
    file: PathBuf,
    source_map: bool,
}

impl PluginModuleCompiler {
    pub fn new(file: impl Into<PathBuf>) -> Self {
        Self {
            file: file.into(),
            source_map: false,
        }
    }

    pub fn with_source_map(mut self, source_map: bool) -> Self {
        self.source_map = source_map;
        self
    }

    /// Stable cache identity for the configured plugin module itself. Project
    /// plugin dependencies are already covered by BuildSession fingerprints;
    /// this also covers plugin files configured outside the project tree.
    pub fn cache_identity(&self) -> Result<String, PluginModuleCompileError> {
        let file = self.absolute_file()?;
        let source = fs_err::read_to_string(&file).map_err(|source| {
            PluginModuleCompileError::ReadSource {
                file: file.clone(),
                source,
            }
        })?;
        let path = ArtifactPathSerializer::path_to_slash_string(&file)
            .map_err(|_| PluginModuleCompileError::NonUtf8Path { file: file.clone() })?;
        Ok(format!(
            "{path}#{}",
            CacheFingerprint::content_hash(&source)
        ))
    }

    /// Produce one ES2016 CommonJS module for Node's `Module._compile`.
    pub async fn compile(self) -> Result<String, PluginModuleCompileError> {
        let file = self.absolute_file()?;
        let mut bundler = rolldown::Bundler::new(self.options(&file)?)
            .map_err(|error| self.rolldown_error(error))?;
        let generated = bundler.generate().await;
        let closed = bundler.close().await;
        let output = generated.map_err(|error| self.rolldown_error(error))?;
        closed.map_err(|error| self.rolldown_error(error))?;

        let mut entry_chunks = output.assets.into_iter().filter_map(|asset| match asset {
            Output::Chunk(chunk) if chunk.is_entry => Some(chunk),
            Output::Chunk(_) | Output::Asset(_) => None,
        });
        let entry = entry_chunks
            .next()
            .ok_or_else(|| PluginModuleCompileError::MissingEntryOutput { file: file.clone() })?;
        if entry_chunks.next().is_some() {
            return Err(PluginModuleCompileError::MultipleEntryOutputs { file });
        }
        Ok(entry.code.clone())
    }

    fn absolute_file(&self) -> Result<PathBuf, PluginModuleCompileError> {
        if self.file.is_absolute() {
            return Ok(self.file.clone());
        }
        std::env::current_dir()
            .map(|cwd| cwd.join(&self.file))
            .map_err(|source| PluginModuleCompileError::CurrentDirectory { source })
    }

    fn options(&self, file: &Path) -> Result<BundlerOptions, PluginModuleCompileError> {
        let file_text = file
            .to_str()
            .ok_or_else(|| PluginModuleCompileError::NonUtf8Path {
                file: file.to_path_buf(),
            })?;
        let output_file = Self::output_file(file);
        let output_file_text =
            output_file
                .to_str()
                .ok_or_else(|| PluginModuleCompileError::NonUtf8Path {
                    file: output_file.clone(),
                })?;
        let cwd = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let mut module_types = FxHashMap::default();
        if let Some(extension) = file.extension().and_then(|extension| extension.to_str()) {
            module_types.insert(extension.to_owned(), Self::module_type(extension));
        }

        Ok(BundlerOptions {
            input: Some(vec![InputItem {
                name: Some("plugin-module".to_owned()),
                import: file_text.to_owned(),
            }]),
            cwd: Some(cwd),
            external: Some(Self::external_dependencies()),
            platform: Some(Platform::Node),
            file: Some(output_file_text.to_owned()),
            format: Some(OutputFormat::Cjs),
            exports: Some(OutputExports::Named),
            es_module: Some(EsModuleFlag::Always),
            sourcemap: self.source_map.then_some(SourceMapType::Inline),
            module_types: Some(module_types),
            treeshake: TreeshakeOptions::Boolean(false),
            dynamic_import_in_cjs: Some(false),
            preserve_entry_signatures: Some(PreserveEntrySignatures::Strict),
            experimental: Some(ExperimentalOptions {
                // Rolldown defaults this experiment on and emits absolute
                // module IDs in `//#region` comments. Those paths make output
                // platform-dependent and are not part of the plugin ABI.
                attach_debug_info: Some(AttachDebugInfo::None),
                ..Default::default()
            }),
            transform: Some(BundlerTransformOptions {
                target: Some(Either::Left("es2016".to_owned())),
                // Plugin modules use the legacy `(prototype, propertyKey)`
                // field-decorator contract supported by existing OHOS build
                // plugins. OXC must select it explicitly.
                decorator: Some(DecoratorOptions {
                    legacy: Some(true),
                    ..Default::default()
                }),
                typescript: Some(TypeScriptOptions::default()),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn module_type(extension: &str) -> ModuleType {
        match extension.to_ascii_lowercase().as_str() {
            "js" | "cjs" | "mjs" => ModuleType::Js,
            "jsx" => ModuleType::Jsx,
            "tsx" => ModuleType::Tsx,
            _ => ModuleType::Ts,
        }
    }

    fn output_file(file: &Path) -> PathBuf {
        match file.extension().and_then(|extension| extension.to_str()) {
            Some(extension)
                if matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "ts" | "tsx" | "mts" | "cts"
                ) =>
            {
                file.with_extension("js")
            }
            Some(_) | None => file.to_path_buf(),
        }
    }

    fn external_dependencies() -> IsExternal {
        type ExternalFuture = Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + 'static>>;
        IsExternal::Fn(Some(Arc::new(
            |specifier, importer, _is_resolved| -> ExternalFuture {
                // Rolldown's built-in OXC runtime plugin embeds transform
                // helpers. They must remain inside this generated CommonJS
                // module; every user import stays external for Node to load.
                let is_runtime_helper = Self::is_oxc_runtime_helper(specifier)
                    || importer.is_some_and(Self::is_oxc_runtime_helper);
                let is_dependency = importer.is_some() && !is_runtime_helper;
                Box::pin(async move { Ok(is_dependency) })
            },
        )))
    }

    fn is_oxc_runtime_helper(specifier: &str) -> bool {
        (specifier.starts_with("@oxc-project/runtime/helpers/")
            || specifier.starts_with("\0@oxc-project+runtime@"))
            && specifier.contains("/helpers/")
    }

    fn rolldown_error(
        &self,
        error: rolldown_error::BatchedBuildDiagnostic,
    ) -> PluginModuleCompileError {
        PluginModuleCompileError::Rolldown {
            file: self.file.clone(),
            message: RolldownBuildDiagnostics::new(error).into_message(),
        }
    }
}

#[derive(Debug, Error)]
pub enum PluginModuleCompileError {
    #[error("failed to resolve the current directory for a plugin module: {source}")]
    CurrentDirectory {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read plugin module {}: {source}", file.display())]
    ReadSource {
        file: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("plugin module path is not valid UTF-8: {}", file.display())]
    NonUtf8Path { file: PathBuf },
    #[error("failed to transpile {}:\n{message}", file.display())]
    Rolldown { file: PathBuf, message: String },
    #[error("failed to transpile {}: Rolldown produced no entry module", file.display())]
    MissingEntryOutput { file: PathBuf },
    #[error("failed to transpile {}: Rolldown produced multiple entry modules", file.display())]
    MultipleEntryOutputs { file: PathBuf },
}
