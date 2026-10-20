//! Minification of processed profiles passed to the package tool.

/// `AbstractGeneratePkgModuleJson.doTaskAction` writes `JSON.stringify` of the
/// processed resource profile. Preserve its insertion order, including unknown
/// fields: the package tool's Fastjson HashMap uses that order for bucket ties.
pub fn minify_package_intermediate_module_json(content: &str) -> Result<String, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(content)?;
    serde_json::to_string(&value)
}
