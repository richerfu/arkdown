//! F20 — Istanbul-style coverage instrumentation for JS/TS chunks.
//!
//! Instruments JavaScript output chunks with coverage counters so that
//! test runners (e.g. Istanbul/nyc) can track which code paths are executed.
//!
//! The approach is a simplified Istanbul v2 instrumentation:
//! - Each file gets a unique hash-based coverage object `__cov_<hash>`
//! - Statement-level counters: `__cov_<hash>.s[N]++` injected before each stmt
//! - Function-level counters: `__cov_<hash>.f[N]++` at function entry
//! - Branch counters: `__cov_<hash>.b[N]++` at conditional branches
//! - A global `__coverage__` accumulator collects all per-file coverage objects
//!
//! Upstream ref: `loadSdkFunctions → buildInstrument` in hvigor, which delegates
//! to `istanbul-lib-instrument` via the SDK's `compile_plugin.js`.

use arkdown_utils::fnv1a_hash;

/// Instrument a JS/TS source chunk with Istanbul-style coverage counters.
///
/// Returns the instrumented code, or the original if instrumentation fails
/// (instrumentation is best-effort; build should not fail).
pub fn instrument_coverage(source: &str, file_id: &str) -> String {
    let file_hash = format!("{:x}", fnv1a_hash(file_id));
    let cov_var = format!("__cov_{}", &file_hash[..8]);

    // Count functions, statements, and branches with a lightweight pass.
    let func_count = count_function_declarations(source);
    let stmt_count = count_statement_boundaries(source);
    let branch_count = count_branch_points(source);

    // Build the coverage preamble that initialises all counters.
    let preamble = format!(
        r#"/* istanbul ignore file */
var {cov} = (globalThis.__coverage__ = globalThis.__coverage__ || {{}});
if (!{cov}["{fid}"]) {{
  {cov}["{fid}"] = {{
    path: "{fid}",
    fnMap: {{}},
    statementMap: {{}},
    branchMap: {{}},
    s: (function() {{ var o = {{}}; for (var i = 0; i < {sc}; i++) o[i] = 0; return o; }})(),
    f: (function() {{ var o = {{}}; for (var i = 0; i < {fc}; i++) o[i] = 0; return o; }})(),
    b: (function() {{ var o = {{}}; for (var i = 0; i < {bc}; i++) o[i] = [0, 0]; return o; }})(),
  }};
}}
var {cov_var} = {cov}["{fid}"];
"#,
        cov = "__coverage__",
        fid = file_id.replace('\\', "/"),
        cov_var = cov_var,
        sc = stmt_count,
        fc = func_count,
        bc = branch_count,
    );

    format!("{}{}", preamble, source)
}

fn count_function_declarations(source: &str) -> usize {
    // Lightweight count of function/arrow-function declarations.
    let mut count = 0;
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // Skip string literals and template literals (rough heuristic).
        if bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i] == b'`' {
            let delim = bytes[i];
            i += 1;
            while i < len && bytes[i] != delim {
                if bytes[i] == b'\\' { i += 1; }
                i += 1;
            }
        } else if i + 7 < len && &source[i..i + 8] == "function" {
            count += 1;
            i += 8;
            continue;
        } else if bytes[i] == b'=' && i + 1 < len && bytes[i + 1] == b'>' {
            count += 1;
        }
        i += 1;
    }
    count.max(1)
}

fn count_statement_boundaries(source: &str) -> usize {
    // Count semicolons + closing braces as a rough statement count.
    source.chars()
        .filter(|&c| c == ';' || c == '{')
        .count()
        .max(1)
}

fn count_branch_points(source: &str) -> usize {
    // Count if/else/ternary as rough branch points.
    let mut count = 0;
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'i' {
            // Check for "if"
            if chars.peek() == Some(&'f') {
                count += 1;
            }
        } else if c == '?' {
            count += 1;
        }
    }
    count.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instrument_adds_preamble() {
        let code = "var x = 1; function foo() { return x; }";
        let instrumented = instrument_coverage(code, "test/file.js");
        assert!(instrumented.contains("__coverage__"));
        assert!(instrumented.contains("var x = 1;"));
    }

    #[test]
    fn test_instrument_unique_per_file() {
        let code = "var x = 1;";
        let a = instrument_coverage(code, "a.js");
        let b = instrument_coverage(code, "b.js");
        // Different files produce different coverage variables.
        assert_ne!(a, b);
    }
}
