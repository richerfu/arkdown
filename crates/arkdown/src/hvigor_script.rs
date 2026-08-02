//! Rust-owned compiler for hvigor TypeScript configuration scripts.
//!
//! The latest installed hvigor (`6.24.3`) compiles every `.ts` and `.mjs`
//! file independently to ES2016 CommonJS before handing the result to
//! Node's `Module._compile`. Arkdown keeps only that final V8 hand-off in
//! JavaScript: OXC strips TypeScript and lowers the target syntax while
//! Rolldown owns CommonJS generation and external-module preservation.
//!
//! Observed owners in the latest installed DevEco Studio are
//! `hvigor/src/base/boot/hooks/require-hook.js::extensionHandler` and
//! `hvigor/src/base/util/options/compile-options.js::compilerOptions`.

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

use crate::diagnostics::RolldownBuildDiagnostics;

/// Compiles one hvigor runtime script without executing JavaScript.
///
/// Imports stay external so Node retains hvigor's per-file module cache,
/// extension handlers, package mocks, and `require` resolution behavior.
/// Rolldown still owns module syntax lowering and CommonJS code generation.
#[derive(Debug, Clone)]
pub struct HvigorScriptCompiler {
    file: PathBuf,
    source_map: bool,
}

impl HvigorScriptCompiler {
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

    /// Produce one ES2016 CommonJS module for Node's `Module._compile`.
    pub async fn compile(self) -> Result<String, HvigorScriptCompileError> {
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
            .ok_or_else(|| HvigorScriptCompileError::MissingEntryOutput { file: file.clone() })?;
        if entry_chunks.next().is_some() {
            return Err(HvigorScriptCompileError::MultipleEntryOutputs { file });
        }
        Ok(entry.code.clone())
    }

    fn absolute_file(&self) -> Result<PathBuf, HvigorScriptCompileError> {
        if self.file.is_absolute() {
            return Ok(self.file.clone());
        }
        std::env::current_dir()
            .map(|cwd| cwd.join(&self.file))
            .map_err(|source| HvigorScriptCompileError::CurrentDirectory { source })
    }

    fn options(&self, file: &Path) -> Result<BundlerOptions, HvigorScriptCompileError> {
        let file_text = file
            .to_str()
            .ok_or_else(|| HvigorScriptCompileError::NonUtf8Path {
                file: file.to_path_buf(),
            })?;
        let output_file = Self::output_file(file);
        let output_file_text =
            output_file
                .to_str()
                .ok_or_else(|| HvigorScriptCompileError::NonUtf8Path {
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
                name: Some("hvigor-script".to_owned()),
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
                // module IDs in `//#region` comments. Hvigor's transpiler does
                // not, and those paths make output platform-dependent.
                attach_debug_info: Some(AttachDebugInfo::None),
                ..Default::default()
            }),
            transform: Some(BundlerTransformOptions {
                target: Some(Either::Left("es2016".to_owned())),
                // TypeScript 4.9.5 transpileModule emits hvigor's field
                // decorators with the legacy `(prototype, propertyKey)`
                // contract even though compile-options.js does not spell out
                // experimentalDecorators. OXC must select that contract
                // explicitly or it discards these TypeScript decorators.
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
            Some(extension) if extension.eq_ignore_ascii_case("ts") => file.with_extension("js"),
            Some(extension) if extension.eq_ignore_ascii_case("tsx") => file.with_extension("js"),
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
    ) -> HvigorScriptCompileError {
        HvigorScriptCompileError::Rolldown {
            file: self.file.clone(),
            message: RolldownBuildDiagnostics::new(error).into_message(),
        }
    }
}

#[derive(Debug, Error)]
pub enum HvigorScriptCompileError {
    #[error("failed to resolve the current directory for a hvigor script: {source}")]
    CurrentDirectory {
        #[source]
        source: std::io::Error,
    },
    #[error("hvigor script path is not valid UTF-8: {}", file.display())]
    NonUtf8Path { file: PathBuf },
    #[error("failed to transpile {}:\n{message}", file.display())]
    Rolldown { file: PathBuf, message: String },
    #[error("failed to transpile {}: Rolldown produced no entry module", file.display())]
    MissingEntryOutput { file: PathBuf },
    #[error("failed to transpile {}: Rolldown produced multiple entry modules", file.display())]
    MultipleEntryOutputs { file: PathBuf },
}
