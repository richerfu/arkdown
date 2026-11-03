use std::path::{Path, PathBuf};

use crate::package::snapshot::PackageSnapshot;
use crate::paths::ModulePathLayout;

/// Startup candidates for a package's declared source boundaries. An entry
/// file never makes its parent directory a recursive source root.
///
/// ets2bundle main.js::getPackageJsonEntryPath selects package entries;
/// ets_checker.ts::resolveModuleNames and Rollup then follow their references.
pub(crate) struct PackageSourceInputs {
    pub(crate) roots: Vec<PathBuf>,
    pub(crate) entries: Vec<PathBuf>,
}

impl PackageSourceInputs {
    pub(crate) fn new(root: &Path, package: Option<&PackageSnapshot>) -> Self {
        let layout = ModulePathLayout::new(root);
        let roots = vec![
            layout.ets_source_root(),
            layout.js_source_root(),
            layout.src_main().join("supervisual"),
        ];
        let mut entries = Vec::new();
        if let Some(package) = package {
            let manifest = &package.package_json;
            for entry in [
                manifest.main.as_deref(),
                manifest.types.as_deref(),
                manifest
                    .extra
                    .get("typings")
                    .and_then(serde_json::Value::as_str),
                manifest
                    .extra
                    .get("module")
                    .and_then(serde_json::Value::as_str),
                manifest
                    .extra
                    .get("jsnext:main")
                    .and_then(serde_json::Value::as_str),
            ]
            .into_iter()
            .flatten()
            .filter(|entry| !entry.is_empty())
            {
                entries.push(root.join(entry));
            }
            entries.extend(
                package
                    .oh_export_files
                    .iter()
                    .flat_map(|files| files.iter())
                    .cloned(),
            );
            // metadata.sourceRoots controls resolver lookup. In particular,
            // ["."] does not request recursive compilation of a package root.
        }
        Self { roots, entries }
    }
}
