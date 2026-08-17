//! Module-source identity: compile eligibility, origin language, output paths.

use std::path::{Path, PathBuf};

use arkdown_core::ArkdownError;

use super::*;

pub(crate) struct HvigorModuleSourceId<'a> {
    pub(crate) module_id: &'a str,
}

impl<'a> HvigorModuleSourceId<'a> {
    pub(crate) fn new(module_id: &'a str) -> Self {
        Self { module_id }
    }

    pub(crate) fn should_compile(&self) -> bool {
        let path = Path::new(self.module_id);
        if !path.has_root() {
            return false;
        }
        if arkdown_path::SourceFileName::new(path).is_declaration() {
            return false;
        }
        matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts" | "js" | "mjs" | "cjs")
        )
    }

    pub(crate) fn origin_source_lang(&self) -> &'static str {
        match Path::new(self.module_id)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("ets") => "ets",
            Some("ts") => "ts",
            _ => "js",
        }
    }
}

pub(crate) struct HvigorModuleSourceOutputPath<'a> {
    pub(crate) output_dir: &'a Path,
    pub(crate) project_root: &'a Path,
    pub(crate) ownership: &'a HvigorChunkSource,
    pub(crate) module_name: &'a str,
    pub(crate) source_file: &'a str,
    pub(crate) module_id: &'a str,
    pub(crate) emits_javascript: bool,
}

impl<'a> HvigorModuleSourceOutputPath<'a> {
    pub(crate) fn new(
        output_dir: &'a Path,
        project_root: &'a Path,
        ownership: &'a HvigorChunkSource,
        module_name: &'a str,
        source_file: &'a str,
        module_id: &'a str,
        emits_javascript: bool,
    ) -> Self {
        Self {
            output_dir,
            project_root,
            ownership,
            module_name,
            source_file,
            module_id,
            emits_javascript,
        }
    }

    pub(crate) fn path(&self) -> Result<PathBuf, ArkdownError> {
        Ok(self.output_dir.join(match &self.ownership.kind {
            HvigorChunkSourceKind::SourceModule => Path::new(self.module_name).join(
                HvigorModuleSourceFileAttribution::project_source_file_for(self.source_file)
                    .with_extension(self.output_extension()),
            ),
            HvigorChunkSourceKind::LocalDependencyModule(_) => {
                self.local_dependency_cache_source_file()
            }
            HvigorChunkSourceKind::PackageModule(_) => self.package_cache_source_file()?,
        }))
    }

    pub(crate) fn local_dependency_cache_source_file(&self) -> PathBuf {
        Path::new(&self.ownership.cache_module_name)
            .join(self.source_file)
            .with_extension(self.output_extension())
    }

    pub(crate) fn package_cache_source_file(&self) -> Result<PathBuf, ArkdownError> {
        let module_path = Path::new(self.module_id);
        let relative = module_path.strip_prefix(self.project_root).map_err(|_| {
            ArkdownError::Bundler(format!(
                "hvigor package module source is outside project root: {} not under {}",
                module_path.display(),
                self.project_root.display()
            ))
        })?;
        Ok(relative.with_extension(self.output_extension()))
    }

    pub(crate) fn output_extension(&self) -> &'static str {
        if self.emits_javascript {
            return "js";
        }
        match Path::new(self.module_id)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("ets" | "ts") => "ts",
            Some("js" | "mjs" | "cjs") => "js",
            _ => "js",
        }
    }
}

pub(crate) struct HvigorModuleSourceFileAttribution;

impl HvigorModuleSourceFileAttribution {
    pub(crate) fn project_source_file_for(source_file: &str) -> PathBuf {
        let source_file_path = Path::new(source_file).with_extension("ts");
        if source_file.starts_with("ets/")
            || source_file.starts_with("js/")
            || source_file.starts_with("mock/")
        {
            Path::new("src").join("main").join(source_file_path)
        } else {
            source_file_path
        }
    }

    pub(crate) fn package_source_file_for(source_file: &str) -> PathBuf {
        let path = Path::new(source_file);
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("ets") => path.with_extension("ts"),
            _ => path.to_path_buf(),
        }
    }
}
