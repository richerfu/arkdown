mod version_table;

pub use version_table::rewrite_imports_for_version;

/// Apply all API-version-gated source transforms.
///
/// Currently only the `@ohos/* → @system.*` rewrite for API < 9 is
/// implemented. Additional rules will be added as the static version table
/// is populated from the OHOS SDK declaration structure.
pub fn apply(source: &str, api_version: u32) -> String {
    rewrite_imports_for_version(source, api_version)
}
