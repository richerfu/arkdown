use arkdown_core::PkgContext;

/// Generate an OHM URL for a resolved file.
///
/// Strips the `pkg_path` prefix and file extension, then formats as:
/// `@bundle:<bundle_name>/<module_name>/<rel_path>`
pub fn generate_ohm_url(path: &std::path::Path, pkg_ctx: &PkgContext) -> String {
    let rel = path.strip_prefix(&pkg_ctx.pkg_path).unwrap_or(path);
    let rel_str = rel.to_string_lossy();
    let rel_no_ext = rel_str
        .trim_end_matches(".ets")
        .trim_end_matches(".ts")
        .trim_end_matches(".js");
    format!(
        "@bundle:{}/{}/{}",
        pkg_ctx.bundle_name, pkg_ctx.module_name, rel_no_ext
    )
}

/// Parse an OHM URL into (bundle, module, path) components.
///
/// `"@bundle:com.example/entry/ets/foo/Bar"` → `("com.example", "entry", "ets/foo/Bar")`
pub fn parse_ohm_url(ohm_url: &str) -> Option<(&str, &str, &str)> {
    let rest = ohm_url.strip_prefix("@bundle:")?;
    let slash1 = rest.find('/')?;
    let bundle = &rest[..slash1];
    let rest = &rest[slash1 + 1..];
    let slash2 = rest.find('/')?;
    let module = &rest[..slash2];
    let path = &rest[slash2 + 1..];
    Some((bundle, module, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ohm_url_round_trip() {
        let url = "@bundle:com.example/entry/ets/foo/Bar";
        let (bundle, module, path) = parse_ohm_url(url).expect("parse failed");
        assert_eq!(bundle, "com.example");
        assert_eq!(module, "entry");
        assert_eq!(path, "ets/foo/Bar");
        let reconstructed = format!("@bundle:{}/{}/{}", bundle, module, path);
        assert_eq!(reconstructed, url);
    }

    #[test]
    fn test_parse_ohm_url_invalid() {
        assert!(parse_ohm_url("not_an_ohm_url").is_none());
        assert!(parse_ohm_url("@bundle:noslash").is_none());
        assert!(parse_ohm_url("@bundle:one/noslash").is_none());
    }

    #[test]
    fn test_parse_ohm_url_with_dotted_bundle_name() {
        // Verify a full real-world OHM URL parses correctly.
        let url = "@bundle:com.example.app/entry/ets/entryability/EntryAbility";
        let (bundle, module, path) = parse_ohm_url(url).expect("should parse");
        assert_eq!(bundle, "com.example.app");
        assert_eq!(module, "entry");
        assert_eq!(path, "ets/entryability/EntryAbility");
        // Verify the three components when split by '/'
        let parts: Vec<&str> = path.split('/').collect();
        assert_eq!(parts, vec!["ets", "entryability", "EntryAbility"]);
    }

    #[test]
    fn test_parse_ohm_url_deeply_nested_path() {
        let url = "@bundle:com.ohos.example/feature/ets/utils/http/client";
        let (bundle, module, path) = parse_ohm_url(url).expect("should parse");
        assert_eq!(bundle, "com.ohos.example");
        assert_eq!(module, "feature");
        assert_eq!(path, "ets/utils/http/client");
    }

    #[test]
    fn test_generate_ohm_url() {
        use std::path::PathBuf;
        let pkg_ctx = PkgContext {
            package_name: "mypackage".to_string(),
            bundle_name: "com.example".to_string(),
            module_name: "entry".to_string(),
            version: "1.0.0".to_string(),
            pkg_path: PathBuf::from("/project/entry"),
            entry_path: None,
        };
        let path = PathBuf::from("/project/entry/ets/pages/Index.ets");
        let url = generate_ohm_url(&path, &pkg_ctx);
        assert_eq!(url, "@bundle:com.example/entry/ets/pages/Index");
    }
}
