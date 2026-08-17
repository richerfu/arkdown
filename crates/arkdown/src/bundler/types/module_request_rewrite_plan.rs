//! Reusable module-request rewrite inventory for coverage retransformation.

use std::borrow::Cow;

use crate::chunk_module_request::HvigorChunkImportMap;
use arkdown_core::ArkdownError;

use crate::bundler::*;

/// Reusable module-request rewrite inventory for a transformed source file.
///
/// The unit-test coverage plugin runs after `etsTransform` but before
/// `genAbc` in hvigor. Arkdown invokes the SDK-owned coverage transformer at
/// the equivalent boundary and then replays the exact request rewrites that
/// the initial Rolldown graph resolved for the module. Keeping this plan next
/// to [`HvigorModuleSourceImportRewriter`] ensures both paths share the same
/// ArkTS parser and literal-emission rules.
pub struct HvigorModuleRequestRewritePlan {
    pub(crate) module_id: String,
    pub(crate) imports: HvigorChunkImportMap,
}

impl HvigorModuleRequestRewritePlan {
    pub fn from_baseline(
        module_id: impl Into<String>,
        transformed_source: &str,
        rewritten_source: &str,
    ) -> Result<Self, ArkdownError> {
        let module_id = module_id.into();
        let transformed_requests =
            HvigorModuleRequestSpecifiers::new(transformed_source, &module_id).collect_literals();
        let rewritten_requests =
            HvigorModuleRequestSpecifiers::new(rewritten_source, &module_id).collect_literals();
        if transformed_requests.len() != rewritten_requests.len() {
            return Err(ArkdownError::Bundler(format!(
                "cannot capture module-request rewrites for '{module_id}': transformed source has {} requests but materialized source has {}",
                transformed_requests.len(),
                rewritten_requests.len()
            )));
        }

        let mut imports = HvigorChunkImportMap::default();
        for (transformed, rewritten) in transformed_requests.iter().zip(&rewritten_requests) {
            if transformed.specifier == rewritten.specifier {
                continue;
            }
            if let Some(previous) =
                imports.insert(transformed.specifier.clone(), rewritten.specifier.clone())
            {
                if previous != rewritten.specifier {
                    return Err(ArkdownError::Bundler(format!(
                        "module request '{}' in '{module_id}' resolved to both '{}' and '{}'",
                        transformed.specifier, previous, rewritten.specifier
                    )));
                }
            }
        }
        Ok(Self { module_id, imports })
    }

    pub fn rewrite<'a>(&'a self, source: &'a str) -> Cow<'a, str> {
        HvigorModuleSourceImportRewriter::new(source, &self.module_id, &self.imports).rewrite()
    }
}
