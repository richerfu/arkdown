use std::path::Path;

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
