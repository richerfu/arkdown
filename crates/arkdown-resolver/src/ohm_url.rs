use arkdown_core::PkgContext;
use arkdown_path::OhmUrl;

/// Builds runtime OHM URLs for resolved files.
pub struct OhmUrlBuilder;

impl OhmUrlBuilder {
    /// Generate an OHM URL for a resolved file.
    ///
    /// Strips the `pkg_path` prefix and file extension, then formats as:
    /// `@bundle:<bundle_name>/<module_name>/<rel_path>`.
    pub fn generate(path: &std::path::Path, pkg_ctx: &PkgContext) -> String {
        pkg_ctx.to_ohm_url(path)
    }
}

/// Parsed OHM URL components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OhmUrlParseResult<'a> {
    pub bundle: &'a str,
    pub module: &'a str,
    pub path: &'a str,
}

impl<'a> OhmUrlParseResult<'a> {
    /// Parse an OHM URL into bundle, module, and path components.
    ///
    /// `"@bundle:com.example/entry/ets/foo/Bar"` parses to bundle
    /// `"com.example"`, module `"entry"`, and path `"ets/foo/Bar"`.
    pub fn parse(ohm_url: &'a str) -> Option<Self> {
        let parts = OhmUrl::parse_bundle_parts(ohm_url)?;
        Some(Self {
            bundle: parts.bundle_name,
            module: parts.module_name,
            path: parts.path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ohm_url_round_trip() {
        let url = "@bundle:com.example/entry/ets/foo/Bar";
        let parsed = OhmUrlParseResult::parse(url).expect("parse failed");
        assert_eq!(parsed.bundle, "com.example");
        assert_eq!(parsed.module, "entry");
        assert_eq!(parsed.path, "ets/foo/Bar");
        let reconstructed = format!(
            "@bundle:{}/{}/{}",
            parsed.bundle, parsed.module, parsed.path
        );
        assert_eq!(reconstructed, url);
    }

    #[test]
    fn test_parse_ohm_url_invalid() {
        assert!(OhmUrlParseResult::parse("not_an_ohm_url").is_none());
        assert!(OhmUrlParseResult::parse("@bundle:noslash").is_none());
        assert!(OhmUrlParseResult::parse("@bundle:one/noslash").is_none());
    }

    #[test]
    fn test_parse_ohm_url_with_dotted_bundle_name() {
        // Verify a full real-world OHM URL parses correctly.
        let url = "@bundle:com.example.app/entry/ets/entryability/EntryAbility";
        let parsed = OhmUrlParseResult::parse(url).expect("should parse");
        assert_eq!(parsed.bundle, "com.example.app");
        assert_eq!(parsed.module, "entry");
        assert_eq!(parsed.path, "ets/entryability/EntryAbility");
        // Verify the three components when split by '/'
        let parts: Vec<&str> = parsed.path.split('/').collect();
        assert_eq!(parts, vec!["ets", "entryability", "EntryAbility"]);
    }

    #[test]
    fn test_parse_ohm_url_deeply_nested_path() {
        let url = "@bundle:com.ohos.example/feature/ets/utils/http/client";
        let parsed = OhmUrlParseResult::parse(url).expect("should parse");
        assert_eq!(parsed.bundle, "com.ohos.example");
        assert_eq!(parsed.module, "feature");
        assert_eq!(parsed.path, "ets/utils/http/client");
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
        let url = OhmUrlBuilder::generate(&path, &pkg_ctx);
        assert_eq!(url, "@bundle:com.example/entry/ets/pages/Index");
    }
}
