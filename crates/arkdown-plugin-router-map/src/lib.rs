use std::sync::{Arc, Mutex};

use arkdown_core::{ArkdownPlugin, CacheContext, CachedModule};
use async_trait::async_trait;

/// Plugin that triggers cache invalidation when the router map JSON changes.
///
/// The router map is a JSON file listing named routes and their source files,
/// enabling type-safe navigation in ArkTS. When its content changes, any
/// cached module entry for the router map file itself must be invalidated.
pub struct RouterMapPlugin {
    /// FNV-1a hash of the current router map JSON content.
    /// `None` means no router map has been observed yet — never invalidate.
    current_hash: Arc<Mutex<Option<u64>>>,
}

impl RouterMapPlugin {
    pub fn new() -> Self {
        RouterMapPlugin {
            current_hash: Arc::new(Mutex::new(None)),
        }
    }

    /// Update the stored router map hash from its raw JSON content.
    /// Call this whenever the router map file is written by `arkdown-profile`.
    pub fn set_router_map_content(&self, content: &str) {
        let hash = fnv1a_hash(content);
        *self.current_hash.lock().unwrap() = Some(hash);
    }

    /// Update the stored hash directly (e.g. from a pre-computed value).
    pub fn set_router_map_hash(&self, hash: u64) {
        *self.current_hash.lock().unwrap() = Some(hash);
    }
}

impl Default for RouterMapPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ArkdownPlugin for RouterMapPlugin {
    fn name(&self) -> &str {
        "router-map"
    }

    /// Returns `true` when:
    /// - A router map hash has been set (i.e., a router map exists), AND
    /// - The cached module ID refers to the router map file itself.
    ///
    /// This ensures that `router_map.json` is re-processed whenever its
    /// content changes, which in turn re-invalidates any modules that depend on it.
    fn should_invalidate_cache(&self, _ctx: &CacheContext, cached: &CachedModule) -> bool {
        let current = *self.current_hash.lock().unwrap();
        match current {
            // No router map observed yet — nothing to invalidate.
            None => false,
            Some(_hash) => {
                // Invalidate the router map entry itself so it gets re-loaded.
                // Callers that index dependents can then propagate transitively.
                cached.id.ends_with("router_map.json")
            }
        }
    }
}

/// FNV-1a 64-bit hash.
fn fnv1a_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_invalidation_without_hash() {
        let plugin = RouterMapPlugin::new();
        let cached = CachedModule {
            id: "index.ets".to_string(),
            source_hash: String::new(),
        };
        assert!(!plugin.should_invalidate_cache(&CacheContext::default(), &cached));
    }

    #[test]
    fn test_no_invalidation_for_non_router_map_module() {
        let plugin = RouterMapPlugin::new();
        plugin.set_router_map_content(r#"{"routes":[]}"#);
        let cached = CachedModule {
            id: "src/pages/Index.ets".to_string(),
            source_hash: String::new(),
        };
        // Non-router-map modules are not invalidated by this plugin.
        assert!(!plugin.should_invalidate_cache(&CacheContext::default(), &cached));
    }

    #[test]
    fn test_invalidates_router_map_module_when_hash_set() {
        let plugin = RouterMapPlugin::new();
        plugin.set_router_map_content(
            r#"{"routes":[{"name":"Index","pageSourceFile":"pages/Index.ets"}]}"#,
        );
        let cached = CachedModule {
            id: "build/default/cache/router_map.json".to_string(),
            source_hash: String::new(),
        };
        assert!(plugin.should_invalidate_cache(&CacheContext::default(), &cached));
    }

    #[test]
    fn test_fnv1a_hash_deterministic() {
        assert_eq!(fnv1a_hash("hello"), fnv1a_hash("hello"));
        assert_ne!(fnv1a_hash("hello"), fnv1a_hash("world"));
    }
}
