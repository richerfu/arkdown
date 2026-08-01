use std::path::{Component, Path, PathBuf};

use crate::{lexical_normalize, ArtifactPathSerializer};

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

    /// Compute SDK `registerNamedRoute.pagePath`.
    ///
    /// Mirrors `process_module_package.js::pathMessage`, which evaluates
    /// `path.relative(projectConfig.projectPath, resourceFileName)` before
    /// removing the `.ets` suffix. `project_path` is hvigor's `aceModuleRoot`
    /// (`<module>/src/main/ets`), while relative Rolldown IDs are rooted at
    /// `project_root`.
    pub fn entry_page_path(&self, project_path: &Path, project_root: &Path) -> String {
        let source_path = Path::new(self.id);
        let source_path = if source_path.has_root() {
            source_path.to_path_buf()
        } else {
            project_root.join(source_path)
        };
        let relative = Self::relative_path(project_path, &source_path);
        Self::path_without_extension(&relative, Self::ETS_SEGMENT)
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

    /// Platform-native lexical equivalent of Node's `path.relative`.
    fn relative_path(from: &Path, to: &Path) -> PathBuf {
        let from = lexical_normalize(from);
        let to = lexical_normalize(to);
        let from_components = from.components().collect::<Vec<_>>();
        let to_components = to.components().collect::<Vec<_>>();
        if !Self::same_path_anchor(&from_components, &to_components) {
            return to;
        }

        let common = from_components
            .iter()
            .zip(&to_components)
            .take_while(|(left, right)| left == right)
            .count();
        let mut relative = PathBuf::new();
        for component in &from_components[common..] {
            if matches!(component, Component::Normal(_)) {
                relative.push("..");
            }
        }
        for component in &to_components[common..] {
            relative.push(component.as_os_str());
        }
        relative
    }

    fn same_path_anchor(left: &[Component<'_>], right: &[Component<'_>]) -> bool {
        match (left.first(), right.first()) {
            (Some(Component::Prefix(left)), Some(Component::Prefix(right))) => left == right,
            (Some(Component::RootDir), Some(Component::RootDir)) => true,
            (Some(Component::Prefix(_)), _) | (_, Some(Component::Prefix(_))) => false,
            (Some(Component::RootDir), _) | (_, Some(Component::RootDir)) => false,
            _ => true,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_page_path_is_relative_to_hvigor_project_path() {
        let page = HvigorModulePath::new("/project/meituan/src/main/ets/pages/Index.ets");

        assert_eq!(
            page.entry_page_path(
                Path::new("/project/meituan/src/main/ets"),
                Path::new("/project"),
            ),
            "pages/Index"
        );
    }

    #[test]
    fn dependency_entry_page_path_keeps_package_identity() {
        let page = HvigorModulePath::new(
            "/project/oh_modules/.ohpm/appmock@1.0.0/oh_modules/appmock/src/main/ets/pages/Index.ets",
        );

        assert_eq!(
            page.entry_page_path(
                Path::new("/project/meituan/src/main/ets"),
                Path::new("/project"),
            ),
            "../../../../oh_modules/.ohpm/appmock@1.0.0/oh_modules/appmock/src/main/ets/pages/Index"
        );
    }

    #[test]
    fn relative_rolldown_id_is_resolved_from_project_root() {
        let page = HvigorModulePath::new(
            "oh_modules/.ohpm/appmock@1.0.0/oh_modules/appmock/src/main/ets/pages/Index.ets",
        );

        assert_eq!(
            page.entry_page_path(
                Path::new("/project/meituan/src/main/ets"),
                Path::new("/project"),
            ),
            "../../../../oh_modules/.ohpm/appmock@1.0.0/oh_modules/appmock/src/main/ets/pages/Index"
        );
    }

    #[cfg(windows)]
    #[test]
    fn dependency_entry_page_path_uses_slashes_on_windows() {
        let page =
            HvigorModulePath::new(r"C:\project\oh_modules\appmock\src\main\ets\pages\Index.ets");

        assert_eq!(
            page.entry_page_path(
                Path::new(r"C:\project\meituan\src\main\ets"),
                Path::new(r"C:\project"),
            ),
            "../../../../oh_modules/appmock/src/main/ets/pages/Index"
        );
    }
}
