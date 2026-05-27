/// Describes a `@Builder` method extracted from a `@Component` struct.
///
/// In ArkTS, `@Builder` methods are struct instance methods that are desugared
/// into callable builder functions. During output generation they remain as
/// class methods on the ViewPU subclass, but their bodies are desugared the
/// same way as `build()` bodies.
#[derive(Debug, Clone)]
pub struct BuilderMethod {
    pub name: String,
    pub params: String,
    pub body: String,
}

/// Generate the TypeScript class method text for a `@Builder` method.
///
/// The method body is a build() DSL body and will be desugared in v2.
/// For now we emit it as-is with a TODO comment.
pub fn generate_builder_method(method: &BuilderMethod) -> String {
    format!(
        "  {}({}): void {{\n    // TODO: desugar @Builder body\n    {}\n  }}\n",
        method.name,
        method.params,
        method.body.replace('\n', "\n    ")
    )
}
