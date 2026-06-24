//! Raw schema for the router map profile JSON.

use serde::{Deserialize, Serialize};

/// Top-level router map profile.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawRouterMap {
    /// Route entries.
    #[serde(default)]
    pub route_map: Vec<RawRouteEntry>,
}

/// A single route entry in the router map.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRouteEntry {
    /// Route name (page component name).
    pub name: String,
    /// Source file path (relative to `src/main`).
    #[serde(default)]
    pub page_source_file: String,
    /// Runtime build function name.
    #[serde(default)]
    pub build_function: String,
    /// Custom data attached to the route.
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}
