//! Integration tests for the minification pass.
//!
//! Verifies that `-compact` triggers `oxc_minifier` and that the result is
//! shorter than the input while remaining valid JavaScript.

use arkdown_obfuscate::{ObfuscationOptions, Obfuscator};
use tempfile::tempdir;

fn make_obfuscator_with_rules(rules: &str) -> (Obfuscator, tempfile::TempDir) {
    let tmp = tempdir().expect("tempdir");
    let rule = tmp.path().join("rules.txt");
    std::fs::write(&rule, rules.as_bytes()).expect("write rule file");
    let ob = Obfuscator::new(ObfuscationOptions {
        rule_files: vec![rule],
        cache_dir: tmp.path().to_path_buf(),
        export_rule_path: None,
    })
    .expect("Obfuscator::new");
    (ob, tmp)
}

#[test]
fn minify_output_is_shorter_than_input() {
    let (ob, _tmp) = make_obfuscator_with_rules("-enable-obfuscation\n-compact\n");
    let src = concat!(
        "var longVariableName = 1;\n",
        "var anotherLongVariableName = longVariableName + 2;\n",
        "var yetAnotherLongVariableName = anotherLongVariableName * 3;\n",
    );
    let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
    assert!(
        out.code.len() < src.len(),
        "minified output ({} bytes) should be shorter than input ({} bytes)\noutput: {}",
        out.code.len(),
        src.len(),
        out.code,
    );
}

#[test]
fn minify_dead_branch_is_removed() {
    let (ob, _tmp) = make_obfuscator_with_rules("-enable-obfuscation\n-compact\n");
    let src = concat!(
        "var x = 1;\n",
        "if (false) {\n",
        "    var deadCode = 'never runs';\n",
        "}\n",
        "var y = x + 1;\n",
    );
    let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
    assert!(
        !out.code.contains("never runs"),
        "dead string literal should be eliminated: {}",
        out.code
    );
}

#[test]
fn minify_output_is_valid_js() {
    let (ob, _tmp) = make_obfuscator_with_rules("-enable-obfuscation\n-compact\n");
    let src = concat!(
        "function greet(name) { return 'Hello, ' + name; }\n",
        "var result = greet('World');\n",
        "var unused = result.length + 100;\n",
    );
    let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
    assert!(!out.code.is_empty(), "output should not be empty");

    use oxc_allocator::Allocator;
    use oxc_parser::{Parser, ParserReturn};
    use oxc_span::SourceType;

    let alloc = Allocator::default();
    let source_type = SourceType::default().with_module(true);
    let ParserReturn { diagnostics, .. } = Parser::new(&alloc, &out.code, source_type).parse();
    assert!(
        diagnostics.is_empty(),
        "minified output must parse without errors: {:?}\noutput: {}",
        diagnostics,
        out.code,
    );
}

#[test]
fn compact_without_enable_obfuscation_still_minifies() {
    let (ob, _tmp) = make_obfuscator_with_rules("-compact\n");
    let src = concat!(
        "var longVariableName = 100;\n",
        "var anotherLongName   = longVariableName + 200;\n",
    );
    let out = ob.obfuscate_source(src, "test.js").expect("obfuscate");
    assert!(
        out.code.len() < src.len(),
        "compact alone should shrink source: {} vs {} bytes\noutput: {}",
        out.code.len(),
        src.len(),
        out.code,
    );
}
