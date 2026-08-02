use arkdown_config::BuildScriptPluginPathResolver;

#[test]
fn resolver_uses_latest_hvigor_plugin_extension_order() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(temp.path().join("plugin.ts"), "export default {}")
        .expect("write TypeScript plugin");
    std::fs::write(temp.path().join("plugin.js"), "exports.default = {}")
        .expect("write JavaScript plugin");

    let resolver = BuildScriptPluginPathResolver::new(temp.path());
    assert_eq!(
        resolver.resolve("./plugin").expect("resolved plugin"),
        temp.path().join("plugin.js")
    );
    assert_eq!(
        resolver
            .resolve("./plugin.ts")
            .expect("explicit TypeScript plugin"),
        temp.path().join("plugin.ts")
    );
}

#[test]
fn resolver_preserves_logical_absolute_paths_and_missing_candidates() {
    let temp = tempfile::tempdir().expect("tempdir");
    let resolver = BuildScriptPluginPathResolver::new(temp.path());
    let missing = temp.path().join("nested/../missing-plugin");

    assert_eq!(
        resolver
            .resolve(&missing.to_string_lossy())
            .expect("missing path remains loadable for Node diagnostic"),
        temp.path().join("missing-plugin")
    );
    assert!(resolver.resolve("").is_none());
}
