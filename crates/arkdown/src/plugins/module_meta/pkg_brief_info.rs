use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

pub use arkdown_core::PkgBriefInfo;
use arkdown_path::{lexical_normalize, ArtifactPathSerializer, OhmUrl};

use super::meta_compute::ModuleMetaComputer;

/// Builds the `pkg_name → PkgBriefInfo` map from resolved module IDs.
pub struct PkgBriefInfoBuilder<'a> {
    module_paths: &'a [String],
}

impl<'a> PkgBriefInfoBuilder<'a> {
    pub fn new(module_paths: &'a [String]) -> Self {
        Self { module_paths }
    }

    /// OHM-URL identifiers (`@bundle:` / `@shared:`) are skipped because they
    /// are external references without a local `oh-package.json5`.
    pub fn build(&self) -> FxHashMap<String, PkgBriefInfo> {
        let mut map: FxHashMap<String, PkgBriefInfo> = FxHashMap::default();

        for id in self.module_paths {
            if OhmUrl::parse(id)
                .ok()
                .is_some_and(|url| url.is_cross_bundle_reference())
            {
                continue;
            }

            if let Some(pkg) = ModuleMetaComputer::find_pkg_root(Path::new(id), 6) {
                let pkg_name = pkg.name().map(str::to_string).unwrap_or_else(|| {
                    pkg.pkg_path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                });

                if let Some(pkg_root) = PkgBriefInfoPath::new(pkg.pkg_path()).absolute_string() {
                    map.entry(pkg_name.clone()).or_insert_with(|| PkgBriefInfo {
                        pkg_root,
                        pkg_name,
                        source_roots: vec!["src/main/ets".to_string()],
                        original_source_roots: vec![],
                    });
                }
            }
        }

        map
    }
}

struct PkgBriefInfoPath<'a> {
    path: &'a Path,
}

impl<'a> PkgBriefInfoPath<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn absolute_string(&self) -> Option<String> {
        let absolute = self.absolute_path();
        ArtifactPathSerializer::path_to_slash_string(&absolute).ok()
    }

    fn absolute_path(&self) -> PathBuf {
        if let Ok(path) = std::fs::canonicalize(self.path) {
            return path;
        }
        if self.path.is_absolute() {
            return lexical_normalize(self.path);
        }
        lexical_normalize(
            &std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(self.path),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn test_build_pkg_brief_info_skips_ohm_urls() {
        let paths = vec![
            "@bundle:com.example/entry/ets/pages/Index".to_string(),
            "@shared:com.example/hsp/ets/utils/Helper".to_string(),
        ];
        let result = PkgBriefInfoBuilder::new(&paths).build();
        assert!(result.is_empty());
    }

    #[test]
    fn test_build_pkg_brief_info_with_package() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("oh-package.json5");
        let mut f = std::fs::File::create(&pkg_path).unwrap();
        writeln!(f, r#"{{ "name": "test-lib", "version": "0.1.0" }}"#).unwrap();

        let src_dir = dir.path().join("src").join("main").join("ets");
        std::fs::create_dir_all(&src_dir).unwrap();
        let src_file = src_dir.join("index.ets");
        std::fs::write(&src_file, "export {}").unwrap();

        let paths = vec![src_file.to_string_lossy().to_string()];
        let result = PkgBriefInfoBuilder::new(&paths).build();

        assert!(result.contains_key("test-lib"));
        let info = &result["test-lib"];
        assert_eq!(info.source_roots, vec!["src/main/ets"]);
    }
}
