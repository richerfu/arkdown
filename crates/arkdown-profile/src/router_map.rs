/// A single entry in the router map — associates a named route with the
/// source file that provides its `@Entry`-decorated struct.
#[derive(Debug, Clone)]
pub struct RouterMapEntry {
    /// Route name (the `routeName` parameter from `@Entry`).
    pub route_name: String,
    /// Absolute path to the source `.ets` file.
    pub src_path: String,
    /// OHM URL identifier used in the bundle (if `useNormalizedOHMUrl` is active).
    pub ohm_url: Option<String>,
}

/// Router map processor for a single OHOS module.
///
/// `RouterMapProcessor` is the named-type API for router map operations.
/// It collects, validates, and writes router map entries for the
/// `ProcessRouterMap` build stage.
///
/// # Upstream correspondence
///
/// Corresponds to `process-router-map.js::RouterMapProcessor` and the
/// `router-map-plugin.js` router map collection in hvigor.
#[derive(Debug, Default)]
pub struct RouterMapProcessor {
    entries: Vec<RouterMapEntry>,
}

impl RouterMapProcessor {
    /// Create a new processor with no entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect router map entries from pre-discovered input.
    ///
    /// Filters and validates the entries, preserving only those with valid
    /// route names and source paths.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `process-router-map.js::collectRouterEntries` and
    /// `router-map-plugin.js::buildIndex`.
    pub fn collect_router_entries(&self, entries: Vec<RouterMapEntry>) -> Vec<RouterMapEntry> {
        entries
            .into_iter()
            .filter(|e| !e.route_name.is_empty() && !e.src_path.is_empty())
            .collect()
    }

    /// Write the collected router map to the output path.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `process-router-map.js::RouterMapProcessor::write`.
    pub fn write(&self, output_path: &std::path::Path) -> std::io::Result<()> {
        let entries_json: Vec<serde_json::Value> = self
            .entries
            .iter()
            .map(|e| {
                let mut obj = serde_json::json!({
                    "name": e.route_name,
                    "pageSourceFile": e.src_path,
                });
                if let Some(ohm) = &e.ohm_url {
                    obj["ohmUrl"] = serde_json::Value::String(ohm.clone());
                }
                obj
            })
            .collect();
        let content = serde_json::json!({ "routerMap": entries_json });
        let json = serde_json::to_string_pretty(&content).map_err(std::io::Error::other)?;
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(output_path, json)
    }

    /// Compute the fingerprint of the router map entries for cache invalidation.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `router-map-plugin.js` router map hash computation.
    pub fn fingerprint_inputs(&self) -> u64 {
        let mut state = arkdown_core::fnv1a64(&[]);
        for e in &self.entries {
            state = arkdown_core::fnv1a64_fold(state, e.route_name.as_bytes());
            state = arkdown_core::fnv1a64_fold(state, e.src_path.as_bytes());
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_router_entries_passthrough() {
        let entries = vec![
            RouterMapEntry {
                route_name: "Home".to_string(),
                src_path: "/project/entry/src/main/ets/pages/Home.ets".to_string(),
                ohm_url: None,
            },
            RouterMapEntry {
                route_name: "Detail".to_string(),
                src_path: "/project/entry/src/main/ets/pages/Detail.ets".to_string(),
                ohm_url: Some("@bundle:com.example.app/entry/ets/pages/Detail".to_string()),
            },
        ];
        let result = RouterMapProcessor::new().collect_router_entries(entries);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].route_name, "Home");
        assert_eq!(result[1].route_name, "Detail");
        assert_eq!(
            result[1].ohm_url.as_deref(),
            Some("@bundle:com.example.app/entry/ets/pages/Detail")
        );
    }
}
