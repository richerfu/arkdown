use std::path::Path;

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
}
