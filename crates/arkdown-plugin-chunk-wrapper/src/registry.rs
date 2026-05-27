/// Generate the globalThis registry export line appended to each exporting chunk.
///
/// Format:
/// ```js
/// globalThis["fileName_chunkHash"] = globalThis["fileName_chunkHash"] || {};
/// ```
///
/// Returns an empty string if the hash is empty (should not happen in practice).
pub fn generate_registry_export(name: &str, chunk_hash: &str) -> String {
    if chunk_hash.is_empty() {
        return String::new();
    }
    let key = format!("{}_{}", name, chunk_hash);
    format!(
        "globalThis[\"{}\"] = globalThis[\"{}\"] || {{}};\n",
        key, key
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generates_registry_line() {
        let result = generate_registry_export("Entry", "a1b2c3d4");
        assert_eq!(
            result,
            "globalThis[\"Entry_a1b2c3d4\"] = globalThis[\"Entry_a1b2c3d4\"] || {};\n"
        );
    }

    #[test]
    fn test_empty_hash_returns_empty() {
        let result = generate_registry_export("Entry", "");
        assert!(result.is_empty());
    }
}
