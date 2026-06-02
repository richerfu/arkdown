use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct ResourceOutputLayout {
    output_dir: PathBuf,
}

impl ResourceOutputLayout {
    pub(crate) fn new(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.output_dir
    }

    pub(crate) fn resources_dir(&self) -> PathBuf {
        self.output_dir.join("resources")
    }

    pub(crate) fn resources_index(&self) -> PathBuf {
        self.output_dir.join("resources.index")
    }

    pub(crate) fn resource_table(&self) -> PathBuf {
        self.output_dir.join("ResourceTable.txt")
    }

    pub(crate) fn ids_defined(&self) -> PathBuf {
        self.output_dir.join("ids_map").join("id_defined.json")
    }

    pub(crate) fn resource_path(&self, relative_path: &Path) -> PathBuf {
        self.resources_dir().join(relative_path)
    }
}
