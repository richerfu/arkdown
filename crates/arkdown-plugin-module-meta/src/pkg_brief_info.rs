use rustc_hash::FxHashMap;
use std::path::Path;

pub use arkdown_core::PkgBriefInfo;

use crate::meta_compute::find_pkg_root;

/// Build the `pkg_name → PkgBriefInfo` map from a list of resolved module IDs.
///
/// OHM-URL identifiers (`@bundle:` / `@shared:`) are skipped because they are
/// external references without a local `oh-package.json5`.
pub fn build_pkg_brief_info(
    module_paths: &[String],
    _project_root: &Path,
) -> FxHashMap<String, PkgBriefInfo> {
    let mut map: FxHashMap<String, PkgBriefInfo> = FxHashMap::default();

    for id in module_paths {
        if id.starts_with("@bundle:") || id.starts_with("@shared:") {
            continue;
        }

        if let Some(pkg) = find_pkg_root(Path::new(id), 6) {
            let pkg_name = pkg.name().map(str::to_string).unwrap_or_else(|| {
                pkg.pkg_path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });

            map.entry(pkg_name.clone()).or_insert_with(|| PkgBriefInfo {
                pkg_root: pkg.pkg_path().to_string_lossy().to_string(),
                pkg_name,
                source_roots: vec!["src/main/ets".to_string()],
                original_source_roots: vec![],
            });
        }
    }

    map
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
        let result = build_pkg_brief_info(&paths, Path::new("/project"));
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
        let result = build_pkg_brief_info(&paths, Path::new("/project"));

        assert!(result.contains_key("test-lib"));
        let info = &result["test-lib"];
        assert_eq!(info.source_roots, vec!["src/main/ets"]);
    }
}
