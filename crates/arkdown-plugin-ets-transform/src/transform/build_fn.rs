/// Desugars an ArkUI `build()` method body.
///
/// The build() body is a DSL where top-level function calls are ArkUI component
/// factory invocations. Each call `Foo(args) { children }` is rewritten to an
/// `observeComponentCreation2` pair with `Foo.create(args)` and `Foo.pop()`.
///
/// This is the v1 scaffold — the body is passed through as a comment for the
/// type pipeline to continue. Full DSL desugaring is implemented in the
/// `initialRenderFunction` body rewriter (v2).
///
/// # Arguments
/// * `build_body` — raw text slice of the build() method body (braces excluded)
pub fn desugar_build_body(build_body: &str) -> String {
    // v1: emit the build body as a todo comment so downstream TS processing
    // can see the file and continue without crashing.
    // TODO(SPEC-004a v2): implement full observeComponentCreation2 rewriter.
    format!(
        "  // TODO: desugar build() DSL\n  /* build body:\n{}\n  */\n",
        build_body
            .lines()
            .map(|l| format!("    {}", l))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
