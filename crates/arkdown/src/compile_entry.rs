use std::collections::HashMap;
use std::path::Path;

pub fn compile_entry_stem_counts(entries: &[String]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for entry in entries {
        if let Some(stem) = Path::new(entry.as_str())
            .file_stem()
            .and_then(|s| s.to_str())
        {
            *counts.entry(stem.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

pub fn compile_entry_chunk_name(
    entry: &str,
    index: usize,
    stem_counts: &HashMap<String, usize>,
) -> String {
    let Some(stem) = Path::new(entry).file_stem().and_then(|s| s.to_str()) else {
        return format!("entry_{index}");
    };

    if stem_counts.get(stem).copied().unwrap_or(0) <= 1 {
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
