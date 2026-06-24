use std::path::{Path, PathBuf};

use arkdown_path::ArtifactPath;

pub(crate) struct WatchModuleLayout<'a> {
    module_root: &'a Path,
}

impl<'a> WatchModuleLayout<'a> {
    pub(crate) fn new(module_root: &'a Path) -> Self {
        Self { module_root }
    }

    pub(crate) fn ets_dir(&self) -> PathBuf {
        self.src_main().join("ets")
    }

    pub(crate) fn resources_dir(&self) -> PathBuf {
        self.src_main().join("resources")
    }

    fn src_main(&self) -> PathBuf {
        self.module_root.join("src").join("main")
    }
}

pub(crate) struct DevEcoArtifactLayout<'a> {
    module_name: &'a str,
}

impl<'a> DevEcoArtifactLayout<'a> {
    pub(crate) fn new(module_name: &'a str) -> Self {
        Self { module_name }
    }

    pub(crate) fn default_hap(&self) -> String {
        let file_name = ArtifactPath::from_slash(self.module_name).with_extension("hap");
        ArtifactPath::from_slash(self.module_name)
            .join("build")
            .join("outputs")
            .join("default")
            .join(file_name.as_str())
            .into_string()
    }
}
