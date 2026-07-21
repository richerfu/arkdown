use std::path::{Path, PathBuf};

pub struct HvigorSourceFileFilter<'a> {
    path: &'a Path,
}

impl<'a> HvigorSourceFileFilter<'a> {
    pub fn new(path: &'a Path) -> Self {
        Self { path }
    }

    pub fn is_ignored_macos_metadata(&self) -> bool {
        let Some(name) = self.path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        name == ".DS_Store" || name.starts_with("._")
    }
}

pub struct SourceFileName<'a> {
    path: &'a Path,
}

impl<'a> SourceFileName<'a> {
    pub fn new(path: &'a Path) -> Self {
        Self { path }
    }

    pub fn has_extension(&self, expected: &str) -> bool {
        self.path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
    }

    pub fn is_declaration(&self) -> bool {
        if !matches!(
            self.path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.to_ascii_lowercase())
                .as_deref(),
            Some("ts" | "ets")
        ) {
            return false;
        }

        self.path
            .file_stem()
            .and_then(|stem| Path::new(stem).extension())
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("d"))
            .unwrap_or(false)
    }

    /// Runtime JavaScript files that can be paired with this declaration.
    ///
    /// For example, `index.d.ts` describes `index.js`, `index.mjs`, or
    /// `index.cjs`. Keeping this path derivation here avoids scattering
    /// declaration suffix manipulation across resolvers and transforms.
    pub fn declaration_runtime_candidates(&self) -> Option<[PathBuf; 3]> {
        if !self.is_declaration() {
            return None;
        }
        let declaration_stem = self.path.file_stem()?;
        let runtime_stem = Path::new(declaration_stem).file_stem()?;
        let base = self.path.with_file_name(runtime_stem);
        Some([
            base.with_extension("js"),
            base.with_extension("mjs"),
            base.with_extension("cjs"),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hvigor_source_file_filter_skips_macos_metadata_files() {
        assert!(
            HvigorSourceFileFilter::new(Path::new("entry/src/main/ets/._BuildProfile.ets"))
                .is_ignored_macos_metadata()
        );
        assert!(
            HvigorSourceFileFilter::new(Path::new("entry/src/main/ets/.DS_Store"))
                .is_ignored_macos_metadata()
        );
    }

    #[test]
    fn hvigor_source_file_filter_keeps_ohpm_hidden_package_roots() {
        assert!(!HvigorSourceFileFilter::new(Path::new(
            "oh_modules/.ohpm/pkg/oh_modules/pkg/src/main/ets/Index.ets"
        ))
        .is_ignored_macos_metadata());
    }

    #[test]
    fn declaration_runtime_candidates_are_path_based() {
        assert_eq!(
            SourceFileName::new(Path::new("pkg/src/index.d.ts")).declaration_runtime_candidates(),
            Some([
                PathBuf::from("pkg/src/index.js"),
                PathBuf::from("pkg/src/index.mjs"),
                PathBuf::from("pkg/src/index.cjs"),
            ])
        );
        assert!(SourceFileName::new(Path::new("pkg/src/index.ts"))
            .declaration_runtime_candidates()
            .is_none());
    }
}
