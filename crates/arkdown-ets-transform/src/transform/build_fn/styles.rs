use rustc_hash::FxHashMap;

use oxc_ast::ast::FunctionBody;

/// A component-local `@Styles` declaration.
///
/// SDK `process_component_class.js::processComponentMethod` registers these
/// bodies in `INNER_STYLE_FUNCTION` and removes the method from the generated
/// class. Calls in `build()` and `@Builder` bodies are expanded by
/// `process_component_build.js`.
#[derive(Debug, Clone)]
pub(crate) struct StylesMethodInfo<'a> {
    pub(crate) name: String,
    pub(crate) body: Option<&'a FunctionBody<'a>>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct StylesAttributeIndex<'a> {
    bodies: FxHashMap<String, Option<&'a FunctionBody<'a>>>,
}

impl<'a> StylesAttributeIndex<'a> {
    pub(super) fn from_methods(methods: &[StylesMethodInfo<'a>]) -> Self {
        let bodies = methods
            .iter()
            .map(|method| (method.name.clone(), method.body))
            .collect();
        Self { bodies }
    }

    pub(super) fn body(&self, name: &str) -> Option<Option<&'a FunctionBody<'a>>> {
        self.bodies.get(name).copied()
    }
}
