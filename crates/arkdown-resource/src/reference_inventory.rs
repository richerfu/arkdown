//! Build-start resource lookup inventory consumed by the ArkTS transform.
//!
//! The SDK loader reads `ResourceTable.txt` and walks the compiled `rawfile`
//! directory before Rollup transforms source files. This module owns the same
//! filesystem work so Rolldown hooks can use an immutable in-memory snapshot.

use std::path::Path;

use rustc_hash::FxHashSet;

use crate::ResourceError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceReferenceInventory {
    resources: FxHashSet<String>,
    rawfiles: FxHashSet<String>,
    resource_table_available: bool,
    rawfile_root_available: bool,
}

impl ResourceReferenceInventory {
    pub fn from_resource_table(path: &Path) -> Result<Self, ResourceError> {
        let content = fs_err::read_to_string(path)?;
        let mut inventory = Self {
            resource_table_available: true,
            ..Self::default()
        };
        inventory.collect_resource_table(&content);
        inventory.collect_adjacent_rawfiles(path)?;
        Ok(inventory)
    }

    pub fn from_rawfile_dir(path: &Path) -> Result<Self, ResourceError> {
        let mut inventory = Self::default();
        inventory.collect_rawfiles(path, path)?;
        Ok(inventory)
    }

    pub fn resources(&self) -> &FxHashSet<String> {
        &self.resources
    }

    pub fn rawfiles(&self) -> &FxHashSet<String> {
        &self.rawfiles
    }

    pub fn rawfile_root_available(&self) -> bool {
        self.rawfile_root_available
    }

    pub fn resource_table_available(&self) -> bool {
        self.resource_table_available
    }

    fn collect_resource_table(&mut self, content: &str) {
        for line in content.lines() {
            let mut fields = line.split_whitespace();
            let (Some(resource_type), Some(name), Some(id), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            if ResourceTableId::new(id).is_numeric() {
                self.resources
                    .insert(ResourceReferenceKey::new(resource_type, name).into_string());
            }
        }
    }

    fn collect_adjacent_rawfiles(&mut self, resource_table: &Path) -> Result<(), ResourceError> {
        let Some(output_dir) = resource_table.parent() else {
            return Ok(());
        };
        let rawfile_dir = output_dir.join("resources").join("rawfile");
        self.collect_rawfiles(&rawfile_dir, &rawfile_dir)
    }

    fn collect_rawfiles(&mut self, root: &Path, directory: &Path) -> Result<(), ResourceError> {
        if !directory.is_dir() {
            return Ok(());
        }
        if directory == root {
            self.rawfile_root_available = true;
        }
        let mut entries = fs_err::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                self.collect_rawfiles(root, &path)?;
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|error| ResourceError::Parse {
                    file: path.display().to_string(),
                    message: error.to_string(),
                })?;
            self.rawfiles
                .insert(ResourceRawfilePath::new(relative).into_string()?);
        }
        Ok(())
    }
}

struct ResourceTableId<'a> {
    text: &'a str,
}

impl<'a> ResourceTableId<'a> {
    fn new(text: &'a str) -> Self {
        Self { text }
    }

    fn is_numeric(&self) -> bool {
        let text = self.text.trim();
        text.strip_prefix("0x")
            .or_else(|| text.strip_prefix("0X"))
            .is_some_and(|hex| u32::from_str_radix(hex, 16).is_ok())
            || text.parse::<u32>().is_ok()
    }
}

struct ResourceReferenceKey<'a> {
    resource_type: &'a str,
    name: &'a str,
}

impl<'a> ResourceReferenceKey<'a> {
    fn new(resource_type: &'a str, name: &'a str) -> Self {
        Self {
            resource_type,
            name,
        }
    }

    fn into_string(self) -> String {
        let mut key = String::with_capacity(self.resource_type.len() + self.name.len() + 1);
        key.push_str(self.resource_type);
        key.push('.');
        key.push_str(self.name);
        key
    }
}

struct ResourceRawfilePath<'a> {
    path: &'a Path,
}

impl<'a> ResourceRawfilePath<'a> {
    fn new(path: &'a Path) -> Self {
        Self { path }
    }

    fn into_string(self) -> Result<String, ResourceError> {
        let components = self
            .path
            .components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .map(str::to_string)
                    .ok_or_else(|| ResourceError::Parse {
                        file: self.path.display().to_string(),
                        message: "rawfile path is not valid UTF-8".to_string(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(components.join("/"))
    }
}
