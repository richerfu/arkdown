use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A qualifier parsed from a resources sub-directory name (e.g. `"en_US"`, `"dark"`).
/// An empty string represents the `base` qualifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Qualifier(pub String);

/// The category of a resource file, determined by its immediate parent directory name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceCategory {
    Element,
    Media,
    Profile,
}

/// A single resource file discovered under a resources directory.
#[derive(Debug, Clone)]
pub struct ResourceFile {
    /// Qualifier string; empty for `base`.
    pub qualifier: Qualifier,
    /// Category determined from the directory name.
    pub category: ResourceCategory,
    /// Absolute path to the file on disk.
    pub path: PathBuf,
    /// File stem (filename without extension).
    pub name: String,
}

/// Walk `resources_dir` (e.g. `src/main/resources/`) and return every resource file.
///
/// Expected layout:
/// ```text
/// resources/
///   base/
///     element/  ← EntryType::Element
///     media/    ← EntryType::Media
///     profile/  ← EntryType::Profile
///   en_US/
///     element/
///     …
/// ```
pub fn scan_resources(resources_dir: &Path) -> Vec<ResourceFile> {
    let mut files = Vec::new();

    // min_depth(2) skips the qualifier dirs themselves; max_depth(3) avoids going deeper.
    for entry in WalkDir::new(resources_dir).min_depth(2).max_depth(3) {
        let Ok(e) = entry else { continue };
        if !e.file_type().is_file() {
            continue;
        }
        let path = e.path();

        // Immediate parent → category directory name ("element", "media", "profile").
        let category_dir = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Grandparent → qualifier directory name ("base", "en_US", …).
        let qualifier_dir = path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("base");

        let qualifier = if qualifier_dir == "base" {
            Qualifier(String::new())
        } else {
            Qualifier(qualifier_dir.to_string())
        };

        let category = match category_dir {
            "element" => ResourceCategory::Element,
            "media" => ResourceCategory::Media,
            "profile" => ResourceCategory::Profile,
            _ => continue,
        };

        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        files.push(ResourceFile {
            qualifier,
            category,
            path: path.to_path_buf(),
            name,
        });
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_finds_element_and_media() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        let elem_dir = base.join("resources/base/element");
        let media_dir = base.join("resources/base/media");
        fs::create_dir_all(&elem_dir).unwrap();
        fs::create_dir_all(&media_dir).unwrap();
        fs::write(elem_dir.join("string.json"), "{}").unwrap();
        fs::write(media_dir.join("icon.png"), b"\x89PNG").unwrap();

        let files = scan_resources(&base.join("resources"));
        assert_eq!(files.len(), 2);

        let elem = files.iter().find(|f| f.category == ResourceCategory::Element).unwrap();
        assert_eq!(elem.name, "string");
        assert_eq!(elem.qualifier, Qualifier(String::new()));

        let media = files.iter().find(|f| f.category == ResourceCategory::Media).unwrap();
        assert_eq!(media.name, "icon");
    }

    #[test]
    fn test_scan_non_base_qualifier() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("resources/en_US/element");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("string.json"), "{}").unwrap();

        let files = scan_resources(&tmp.path().join("resources"));
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].qualifier, Qualifier("en_US".to_string()));
    }
}
