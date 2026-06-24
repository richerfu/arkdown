#[cfg(test)]
use arkdown_path::ModuleSpecifierText;

#[cfg(test)]
pub struct ApiVersionImportRewriter<'a> {
    source: &'a str,
    api_version: u32,
}

#[cfg(test)]
impl<'a> ApiVersionImportRewriter<'a> {
    pub fn new(source: &'a str, api_version: u32) -> Self {
        Self {
            source,
            api_version,
        }
    }

    /// Rewrite `@ohos/*` import specifiers to the legacy `@system.*` form for
    /// API versions below 9 (FA model).
    ///
    /// Under the FA model (API < 9), OHOS APIs were distributed as `@system.*`
    /// packages rather than the `@ohos/*` namespace used in the Stage model.
    ///
    /// Examples:
    /// - `from '@ohos/hilog'`    → `from "@system.hilog"`
    /// - `from "@ohos/net.http"` → `from "@system.net.http"`
    ///
    /// For API >= 9 no rewrite is performed; both `@ohos/*` and `@kit.*` are
    /// the canonical import forms and are left unchanged.
    pub fn rewrite(&self) -> std::borrow::Cow<'a, str> {
        if self.api_version >= 9 || !self.source.contains("@ohos/") {
            return std::borrow::Cow::Borrowed(self.source);
        }

        crate::StaticModuleSpecifierRewrite::new(
            self.source,
            "arkdown-api-version.ts",
            |specifier: &str| ModuleSpecifierText::new(specifier).legacy_system_specifier(),
        )
        .rewrite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_ast::ast::Statement;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    struct ImportSourceAssertion<'a> {
        source: &'a str,
    }

    impl<'a> ImportSourceAssertion<'a> {
        fn new(source: &'a str) -> Self {
            Self { source }
        }

        fn has_source(&self, expected: &str) -> bool {
            let allocator = Allocator::default();
            let parsed =
                Parser::new(&allocator, self.source, SourceType::ts().with_module(true)).parse();
            parsed.program.body.iter().any(|statement| {
                matches!(
                    statement,
                    Statement::ImportDeclaration(import)
                        if import.source.value.as_str() == expected
                )
            })
        }
    }

    #[test]
    fn test_no_rewrite_for_api9_plus() {
        let src = "import x from '@ohos/hilog';";
        assert_eq!(ApiVersionImportRewriter::new(src, 9).rewrite(), src);
        assert_eq!(ApiVersionImportRewriter::new(src, 12).rewrite(), src);
    }

    #[test]
    fn test_rewrites_ohos_import_for_api8() {
        let src = "import x from '@ohos/net.http';";
        let result = ApiVersionImportRewriter::new(src, 8).rewrite();
        assert!(
            ImportSourceAssertion::new(&result).has_source("@system.net.http"),
            "got: {}",
            result
        );
    }

    #[test]
    fn test_no_rewrite_for_kit_imports() {
        let src = "import x from '@kit.BasicServicesKit';";
        let result = ApiVersionImportRewriter::new(src, 8).rewrite();
        // @kit.* is Stage-model only; FA model has no equivalent, so we leave it alone.
        assert!(
            result.contains("@kit.BasicServicesKit"),
            "kit import must not be rewritten"
        );
    }
}
