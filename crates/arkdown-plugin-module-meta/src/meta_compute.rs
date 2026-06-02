use std::path::Path;

use arkdown_core::ModuleMeta;
use arkdown_resolver::OhPackage;

/// Walk up from `file_path` looking for an `oh-package.json5`.
///
/// Searches up to `max_depth` parent directories. Returns `None` if not found.
pub fn find_pkg_root(file_path: &Path, max_depth: usize) -> Option<OhPackage> {
    let mut dir = file_path.parent()?;
    for _ in 0..max_depth {
        if let Some(pkg) = OhPackage::load_from_root(dir) {
            return Some(pkg);
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    None
}

/// Compute [`ModuleMeta`] for a module at `module_id`.
///
/// - `project_root`: absolute path to the project root (stored in `belong_project_path`).
/// - `module_name`: OHOS module name from `module.json5` (e.g. `"entry"`, `"hsp-lib"`).
pub fn compute_meta(module_id: &str, project_root: &Path, module_name: &str) -> ModuleMeta {
    let path = Path::new(module_id);

    let pkg_info = find_pkg_root(path, 6);

    // A module is a local (non-ohpm) dependency when its path does not pass
    // through an `oh_modules` directory.
    let is_local = !module_id.contains("oh_modules");

    let is_node_entry_file = pkg_info
        .as_ref()
        .and_then(|info| info.main_field())
        .map(|main| {
            // Treat the module as the entry file when its path ends with the
            // `main` field declared in `oh-package.json5`.
            path.to_string_lossy().ends_with(main)
        })
        .unwrap_or(false);

    let (pkg_name, pkg_version, pkg_path) = if let Some(ref info) = pkg_info {
        (
            info.name().map(str::to_string),
            Some(info.version().to_string()).filter(|v| !v.is_empty()),
            Some(info.pkg_path().to_path_buf()),
        )
    } else {
        (None, None, None)
    };

    ModuleMeta {
        pkg_name,
        pkg_version,
        module_name: Some(module_name.to_string()),
        pkg_path,
        is_local_dependency: is_local,
        is_node_entry_file,
        host_modules_info: vec![],
        need_preload_so: false,
        belong_project_path: Some(project_root.to_path_buf()),
        belong_module_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn test_compute_meta_local_module() {
        let meta = compute_meta(
            "/project/entry/src/main/ets/pages/Index.ets",
            Path::new("/project"),
            "entry",
        );
        assert_eq!(meta.module_name, Some("entry".to_string()));
        assert!(meta.is_local_dependency);
    }

    #[test]
    fn test_compute_meta_oh_modules() {
        let meta = compute_meta(
            "/project/oh_modules/my-lib/src/main/ets/index.ets",
            Path::new("/project"),
            "entry",
        );
        assert!(!meta.is_local_dependency);
    }

    #[test]
    fn test_find_pkg_root_reads_json5() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("oh-package.json5");
        let mut f = std::fs::File::create(&pkg_path).unwrap();
        writeln!(
            f,
            r#"{{ "name": "my-pkg", "version": "1.0.0", "main": "src/index.ets" }}"#
        )
        .unwrap();

        // Create a fake source file one level deeper.
        let src_dir = dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        let src_file = src_dir.join("index.ets");
        std::fs::write(&src_file, "export {}").unwrap();

        let info = find_pkg_root(&src_file, 4).expect("should find oh-package.json5");
        assert_eq!(info.name(), Some("my-pkg"));
        assert_eq!(info.version(), "1.0.0");
    }

    #[test]
    fn test_find_pkg_root_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("some.ets");
        std::fs::write(&file, "").unwrap();
        assert!(find_pkg_root(&file, 2).is_none());
    }
}
