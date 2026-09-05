use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

/// Computes chunk names for a set of compile entry paths.
///
/// Constructed once from all entries, then queried per-entry via
/// [`CompileEntryNamer::chunk_name`].  Keeping the stem-collision map on the
/// struct avoids threading it as a separate parameter.
///
/// ```rust,ignore
/// let namer = CompileEntryNamer::new(&loader_json.compile_entry);
/// for (index, entry) in entries.iter().enumerate() {
///     let name = namer.chunk_name(entry, index);
/// }
/// ```
pub struct CompileEntryNamer {
    stem_counts: FxHashMap<String, usize>,
}

impl CompileEntryNamer {
    /// Build the namer from the full list of entry paths.
    pub fn new(entries: &[String]) -> Self {
        let mut stem_counts = FxHashMap::default();
        for entry in entries {
            if let Some(stem) = Path::new(entry.as_str())
                .file_stem()
                .and_then(|s| s.to_str())
            {
                *stem_counts.entry(stem.to_string()).or_insert(0) += 1;
            }
        }
        Self { stem_counts }
    }

    /// Derive a stable chunk name for `entry` at `index`.
    ///
    /// When the file stem is unique across all entries the stem is used
    /// directly.  For collisions the full relative path is sanitised into a
    /// valid identifier.
    pub fn chunk_name(&self, entry: &str, index: usize) -> String {
        if !Path::new(entry).has_root() {
            return Self::chunk_output_stem(entry);
        }

        let Some(stem) = Path::new(entry).file_stem().and_then(|s| s.to_str()) else {
            return CompileEntryFallbackName::new(index).into_string();
        };

        if self.stem_counts.get(stem).copied().unwrap_or(0) <= 1 {
            return stem.to_string();
        }

        let stripped = Self::chunk_path_stem(entry);
        let mut name = String::with_capacity(stripped.len());
        for ch in stripped.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
                name.push(ch);
            } else {
                name.push('_');
            }
        }
        if name.is_empty() {
            CompileEntryFallbackName::new(index).into_string()
        } else {
            name
        }
    }

    fn chunk_output_stem(entry: &str) -> String {
        let stripped = Self::chunk_path_stem(entry);
        let mut path = PathBuf::new();
        let mut components = Path::new(&stripped).components().peekable();
        if matches!(
            components.peek(),
            Some(std::path::Component::Normal(component)) if component.to_str() == Some("ets")
        ) {
            components.next();
        }
        for component in components {
            path.push(component.as_os_str());
        }
        let output_stem = arkdown_path::ArtifactPathSerializer::rel_to_slash(&path);
        if output_stem.is_empty() {
            stripped
        } else {
            output_stem
        }
    }

    fn chunk_path_stem(entry: &str) -> String {
        let mut path = PathBuf::new();
        for component in Path::new(entry).components() {
            if matches!(component, std::path::Component::CurDir) {
                continue;
            }
            path.push(component.as_os_str());
        }
        CompileEntrySourcePath::new(&mut path).strip_source_extension();
        arkdown_path::ArtifactPathSerializer::rel_to_slash(&path)
    }
}

struct CompileEntryFallbackName {
    index: String,
}

impl CompileEntryFallbackName {
    fn new(index: usize) -> Self {
        Self {
            index: index.to_string(),
        }
    }

    fn into_string(self) -> String {
        CompileEntryNameText::from_parts(["entry_", self.index.as_str()])
    }
}

struct CompileEntryNameText;

impl CompileEntryNameText {
    fn from_parts<const N: usize>(parts: [&str; N]) -> String {
        parts.into_iter().collect()
    }
}

struct CompileEntrySourcePath<'a> {
    path: &'a mut PathBuf,
}

impl<'a> CompileEntrySourcePath<'a> {
    fn new(path: &'a mut PathBuf) -> Self {
        Self { path }
    }

    fn strip_source_extension(&mut self) {
        if CompileEntrySourceExtension::from_path(self.path).is_source() {
            self.path.set_extension("");
        }
    }
}

enum CompileEntrySourceExtension {
    ArkTs,
    TypeScript,
    JavaScript,
    Other,
}

impl CompileEntrySourceExtension {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("ets") => Self::ArkTs,
            Some("ts") => Self::TypeScript,
            Some("js") => Self::JavaScript,
            _ => Self::Other,
        }
    }

    fn is_source(&self) -> bool {
        matches!(self, Self::ArkTs | Self::TypeScript | Self::JavaScript)
    }
}

#[cfg(test)]
mod tests {
    use super::CompileEntryNamer;

    #[test]
    fn local_entry_chunk_name_preserves_path_under_ets_root() {
        let namer = CompileEntryNamer::new(&["./ets/entryability/EntryAbility.ets".to_string()]);

        assert_eq!(
            namer.chunk_name("./ets/entryability/EntryAbility.ets", 0),
            "entryability/EntryAbility"
        );
    }

    #[test]
    fn absolute_entry_chunk_name_keeps_existing_collision_policy() {
        let entry = "/workspace/features/foo/src/main/ets/views/ArticleDetailView.ets";
        let namer = CompileEntryNamer::new(&[entry.to_string()]);

        assert_eq!(namer.chunk_name(entry, 0), "ArticleDetailView");
    }
}
