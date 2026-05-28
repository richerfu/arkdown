/// Rewrite `@ohos/*` import specifiers to the legacy `@system.*` form for
/// API versions below 9 (FA model).
///
/// Under the FA model (API < 9), OHOS APIs were distributed as `@system.*`
/// packages rather than the `@ohos/*` namespace used in the Stage model.
///
/// Examples:
/// - `from '@ohos/hilog'`    → `from '@system.hilog'`
/// - `from "@ohos/net.http"` → `from "@system.net.http"`
///
/// For API >= 9 no rewrite is performed; both `@ohos/*` and `@kit.*` are
/// the canonical import forms and are left unchanged.
pub fn rewrite_imports_for_version(source: &str, api_version: u32) -> String {
    if api_version >= 9 {
        return source.to_string();
    }

    // Replace both single-quoted and double-quoted variants.
    source
        .replace("from '@ohos/", "from '@system.")
        .replace("from \"@ohos/", "from \"@system.")
        .replace("import '@ohos/", "import '@system.")
        .replace("import \"@ohos/", "import \"@system.")
        // Side-effect-only imports
        .replace("require('@ohos/", "require('@system.")
        .replace("require(\"@ohos/", "require(\"@system.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_rewrite_for_api9_plus() {
        let src = "import x from '@ohos/hilog';";
        assert_eq!(rewrite_imports_for_version(src, 9), src);
        assert_eq!(rewrite_imports_for_version(src, 12), src);
    }

    #[test]
    fn test_rewrites_single_quote_for_api8() {
        let src = "import x from '@ohos/hilog';";
        let result = rewrite_imports_for_version(src, 8);
        assert!(result.contains("'@system.hilog'"), "got: {}", result);
    }

    #[test]
    fn test_rewrites_double_quote_for_api8() {
        let src = "import x from \"@ohos/net.http\";";
        let result = rewrite_imports_for_version(src, 8);
        assert!(result.contains("\"@system.net.http\""), "got: {}", result);
    }

    #[test]
    fn test_no_rewrite_for_kit_imports() {
        let src = "import x from '@kit.BasicServicesKit';";
        let result = rewrite_imports_for_version(src, 8);
        // @kit.* is Stage-model only; FA model has no equivalent, so we leave it alone.
        assert!(
            result.contains("@kit.BasicServicesKit"),
            "kit import must not be rewritten"
        );
    }
}
