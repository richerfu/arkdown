use rustc_hash::FxHashMap;
use std::path::Path;

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
        let Some(stem) = Path::new(entry).file_stem().and_then(|s| s.to_str()) else {
            return format!("entry_{index}");
        };

        if self.stem_counts.get(stem).copied().unwrap_or(0) <= 1 {
            return stem.to_string();
        }

        let stripped = entry
            .trim_start_matches("./")
            .trim_end_matches(".ets")
            .trim_end_matches(".ts")
            .trim_end_matches(".js");
        let mut name = String::with_capacity(stripped.len());
        for ch in stripped.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
                name.push(ch);
            } else {
                name.push('_');
            }
        }
        if name.is_empty() {
            format!("entry_{index}")
        } else {
            name
        }
    }
}
