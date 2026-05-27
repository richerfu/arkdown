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

/// Collect router map entries from the ArkTS source files of a module.
///
/// In the real hvigor pipeline, the `router_map` task reads
/// `resources/base/profile/router_map.json` and cross-references it with the
/// compiled module to produce the final routing table.  For now this function
/// accepts pre-discovered entries; the `arkdown-plugin-router-map` crate
/// handles the live discovery.
pub fn collect_router_entries(entries: Vec<RouterMapEntry>) -> Vec<RouterMapEntry> {
    entries
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
                ohm_url: Some(
                    "@bundle:com.example.app/entry/ets/pages/Detail".to_string(),
                ),
            },
        ];
        let result = collect_router_entries(entries);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].route_name, "Home");
        assert_eq!(result[1].route_name, "Detail");
        assert_eq!(
            result[1].ohm_url.as_deref(),
            Some("@bundle:com.example.app/entry/ets/pages/Detail")
        );
    }
}
