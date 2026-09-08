use std::sync::Arc;

/// Immutable ArkUI-X collection inputs.
///
/// TypeScript owns all SDK symbol, overload, return-type, and JSDoc resolution.
/// OXC retains only syntax-level import/component collection, while resolved
/// SDK dependency facts arrive through `ArktsTypeFactsSnapshot`.
#[derive(Clone, Debug)]
pub struct SystemApiCheckOptions {
    collect_crossplatform: bool,
    module_prefixes: Arc<[Box<str>]>,
}

impl Default for SystemApiCheckOptions {
    fn default() -> Self {
        Self {
            collect_crossplatform: false,
            module_prefixes: ["ohos", "system", "kit", "arkts"]
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into(),
        }
    }
}

impl SystemApiCheckOptions {
    /// Enable ArkUI-X module/component artifact collection for full builds.
    /// Hvigor skips these artifact inventories in watch and standalone-check
    /// paths even though TypeScript diagnostics remain enabled.
    pub fn with_crossplatform_collection(mut self, enabled: bool) -> Self {
        self.collect_crossplatform = enabled;
        self
    }

    pub fn with_module_prefixes<I, S>(mut self, prefixes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        self.module_prefixes = prefixes
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>()
            .into();
        self
    }

    pub(crate) fn collects_crossplatform(&self) -> bool {
        self.collect_crossplatform
    }

    pub(crate) fn module_prefixes(&self) -> &[Box<str>] {
        &self.module_prefixes
    }
}
