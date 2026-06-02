use std::path::Path;

/// Check whether the router map cache should be invalidated.
///
/// Returns `true` (invalidate) when:
/// - The router map file exists AND its FNV-1a hash differs from the one
///   stored in `cache_path`, OR the cache file does not exist yet.
///
/// Returns `false` (keep cache) when:
/// - The router map does not exist (nothing to invalidate), OR
/// - The stored hash matches the current router map hash.
pub fn should_invalidate(router_map_path: &Path, cache_path: &Path) -> bool {
    let content = match std::fs::read_to_string(router_map_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let current_hash = arkdown_utils::fnv1a_hash(&content);

    let cached: u64 = match std::fs::read_to_string(cache_path) {
        Ok(s) => s.trim().parse().unwrap_or(0),
        Err(_) => return true,
    };

    current_hash != cached
}

/// Persist the current router map hash to the cache file.
///
/// Call this after a successful build to mark the cache as up-to-date.
pub fn update_cache(router_map_path: &Path, cache_path: &Path) -> std::io::Result<()> {
    let content = std::fs::read_to_string(router_map_path)?;
    let hash = arkdown_utils::fnv1a_hash(&content);
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(cache_path, hash.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_no_invalidation_when_router_map_missing() {
        let cache = tempfile::NamedTempFile::new().unwrap();
        let result =
            should_invalidate(Path::new("/nonexistent_router_map_path.json"), cache.path());
        assert!(!result, "missing router map must not trigger invalidation");
    }

    #[test]
    fn test_invalidate_when_cache_missing() {
        let mut router_map = tempfile::NamedTempFile::new().unwrap();
        writeln!(router_map, r#"{{"routerMap":[]}}"#).unwrap();
        let cache_path = std::env::temp_dir().join("arkdown_test_no_such_cache_xyz.txt");
        let _ = std::fs::remove_file(&cache_path);
        let result = should_invalidate(router_map.path(), &cache_path);
        assert!(result, "missing cache must trigger invalidation");
    }

    #[test]
    fn test_no_invalidation_when_hash_matches() {
        let mut router_map = tempfile::NamedTempFile::new().unwrap();
        writeln!(router_map, r#"{{"routerMap":[]}}"#).unwrap();
        let content = std::fs::read_to_string(router_map.path()).unwrap();
        let hash = arkdown_utils::fnv1a_hash(&content);

        let mut cache = tempfile::NamedTempFile::new().unwrap();
        write!(cache, "{}", hash).unwrap();
        cache.flush().unwrap();

        let result = should_invalidate(router_map.path(), cache.path());
        assert!(!result, "matching hash must not trigger invalidation");
    }

    #[test]
    fn test_invalidate_when_content_changed() {
        let mut router_map = tempfile::NamedTempFile::new().unwrap();
        writeln!(router_map, r#"{{"routerMap":[]}}"#).unwrap();

        let mut cache = tempfile::NamedTempFile::new().unwrap();
        write!(cache, "12345678").unwrap();
        cache.flush().unwrap();

        let result = should_invalidate(router_map.path(), cache.path());
        assert!(result, "changed content must trigger invalidation");
    }

    #[test]
    fn test_fnv1a_hash_deterministic() {
        assert_eq!(
            arkdown_utils::fnv1a_hash("hello"),
            arkdown_utils::fnv1a_hash("hello")
        );
        assert_ne!(
            arkdown_utils::fnv1a_hash("hello"),
            arkdown_utils::fnv1a_hash("world")
        );
    }

    #[test]
    fn test_update_cache_roundtrip() {
        let mut router_map = tempfile::NamedTempFile::new().unwrap();
        write!(router_map, "{}", r#"{"routerMap":[{"name":"Index"}]}"#).unwrap();
        let cache = tempfile::NamedTempFile::new().unwrap();

        update_cache(router_map.path(), cache.path()).unwrap();

        let result = should_invalidate(router_map.path(), cache.path());
        assert!(!result, "after update_cache, should not invalidate");
    }
}
