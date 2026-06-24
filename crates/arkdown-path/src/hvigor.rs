use std::path::{Path, PathBuf};

use crate::ArtifactPathSerializer;

/// Hvigor-compatible module path view used by ETS transforms and sourcemap
/// metadata.
pub struct HvigorModulePath<'a> {
    id: &'a str,
}

impl<'a> HvigorModulePath<'a> {
    pub const ETS_SEGMENT: &'static str = "ets";
    const COMMONS_SEGMENT: &'static str = "commons";
    const FEATURES_SEGMENT: &'static str = "features";
    const MAIN_SEGMENT: &'static str = "main";
    const PRODUCTS_SEGMENT: &'static str = "products";
    const SRC_SEGMENT: &'static str = "src";

    pub fn new(id: &'a str) -> Self {
        Self { id }
    }

    pub fn module_name(&self) -> Option<String> {
        let segments = self.normal_segments();
        let src_main_ets_index = Self::src_main_ets_index(&segments)?;
        src_main_ets_index
            .checked_sub(1)
            .and_then(|module_index| segments.get(module_index).cloned())
    }

    pub fn entry_page_path(&self) -> String {
        self.relative_after_src_main_ets()
            .map(|path| Self::path_without_extension(&path, Self::ETS_SEGMENT))
            .unwrap_or_else(|| self.extensionless_path(Self::ETS_SEGMENT))
    }

    pub fn extensionless_path(&self, extension: &str) -> String {
        Self::path_without_extension(Path::new(self.id), extension)
    }

    pub fn hvigor_segment_path(&self) -> Option<String> {
        let mut segment_path = PathBuf::new();
        let mut in_hvigor_segment = false;

        for component in Path::new(self.id).components() {
            let std::path::Component::Normal(segment) = component else {
                continue;
            };

            if !in_hvigor_segment {
                let segment_name = segment.to_str()?;
                if !Self::is_module_segment(segment_name) {
                    continue;
                }
                in_hvigor_segment = true;
            }

            segment_path.push(segment);
        }

        in_hvigor_segment.then(|| ArtifactPathSerializer::rel_to_slash(&segment_path))
    }

    pub fn src_main_ets_index(segments: &[String]) -> Option<usize> {
        segments.windows(3).position(Self::is_src_main_ets_window)
    }

    fn relative_after_src_main_ets(&self) -> Option<PathBuf> {
        let segments = self.normal_segments();
        let start = Self::src_main_ets_index(&segments)? + 3;
        let mut path = PathBuf::new();
        for segment in segments.into_iter().skip(start) {
            path.push(segment);
        }
        Some(path)
    }

    fn path_without_extension(path: &Path, extension: &str) -> String {
        if path.extension().and_then(|value| value.to_str()) != Some(extension) {
            return ArtifactPathSerializer::rel_to_slash(path);
        }

        let Some(file_stem) = path.file_stem() else {
            return ArtifactPathSerializer::rel_to_slash(path);
        };

        let mut extensionless = path.to_path_buf();
        extensionless.set_file_name(file_stem);
        ArtifactPathSerializer::rel_to_slash(&extensionless)
    }

    fn is_module_segment(name: &str) -> bool {
        matches!(
            name,
            Self::PRODUCTS_SEGMENT | Self::FEATURES_SEGMENT | Self::COMMONS_SEGMENT
        )
    }

    fn is_src_main_ets_window(window: &[String]) -> bool {
        matches!(
            window,
            [src, main, ets]
                if src == Self::SRC_SEGMENT
                    && main == Self::MAIN_SEGMENT
                    && ets == Self::ETS_SEGMENT
        )
    }

    fn normal_segments(&self) -> Vec<String> {
        Path::new(self.id)
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(segment) => segment.to_str().map(str::to_string),
                _ => None,
            })
            .collect()
    }
}
