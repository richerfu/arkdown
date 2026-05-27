use arkdown_core::ArkChunk;

/// Generate the globalThis registry export line appended to each exporting chunk.
///
/// Format:
/// ```js
/// globalThis["fileName_chunkHash"] = globalThis["fileName_chunkHash"] || {};
/// ```
///
/// Returns an empty string if the chunk has no hash (should not happen in practice).
pub fn generate_registry_export(chunk: &ArkChunk) -> String {
    if chunk.chunk_hash.is_empty() {
        return String::new();
    }
    let key = format!("{}_{}", chunk.name, chunk.chunk_hash);
    format!(
        "globalThis[\"{}\"] = globalThis[\"{}\"] || {{}};\n",
        key, key
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::ArkChunk;

    fn make_chunk(name: &str, hash: &str) -> ArkChunk {
        ArkChunk {
            name: name.to_string(),
            chunk_hash: hash.to_string(),
            modules: vec![],
            is_entry: true,
            entry_module: None,
            code: String::new(),
            source_map: None,
        }
    }

    #[test]
    fn test_generates_registry_line() {
        let chunk = make_chunk("Entry", "a1b2c3d4");
        let result = generate_registry_export(&chunk);
        assert_eq!(
            result,
            "globalThis[\"Entry_a1b2c3d4\"] = globalThis[\"Entry_a1b2c3d4\"] || {};\n"
        );
    }

    #[test]
    fn test_empty_hash_returns_empty() {
        let chunk = make_chunk("Entry", "");
        let result = generate_registry_export(&chunk);
        assert!(result.is_empty());
    }
}
