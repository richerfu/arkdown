//! Core obfuscation transform: parse with oxc, rename top-level bindings,
//! optionally strip console.log calls.
//!
//! ## Strategy
//!
//! 1. Parse the source with `oxc_parser`.
//! 2. Walk the top-level `program.body` to collect the names of all declared
//!    bindings (`var`/`let`/`const`, `function`, `class`).
//! 3. Build a rename map: `original → short_name`, honouring the
//!    `-keep-global-name` list.
//! 4. Find **all** word-boundary occurrences of each renamed identifier in the
//!    source text (excluding property accesses, i.e. names preceded by `.`).
//! 5. Apply replacements in descending offset order (so earlier offsets are
//!    not invalidated by shorter/longer replacements).
//!
//! This approach is deliberately simple — it handles IIFE-wrapped CJS bundles
//! produced by `arkdown` where the AST complexity is bounded.

use oxc_allocator::Allocator;
use oxc_ast::ast::{BindingPattern, Statement};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::rename::NameAllocator;
use crate::rules::ObfuscationRules;
use crate::ObfuscationError;

/// Output of a single obfuscation pass.
pub struct ObfuscationOutput {
    /// The transformed JavaScript source.
    pub code: String,
    /// Source-map JSON string, if one was generated.  Currently always `None`
    /// (source-map generation is a future enhancement).
    pub source_map: Option<String>,
}

/// Obfuscate `source` according to `rules`.
///
/// Returns the original source unchanged when `rules.enable_obfuscation` is
/// `false` (callers should check [`crate::should_obfuscate`] before calling,
/// but this function is safe to call unconditionally).
pub fn obfuscate_source(
    source: &str,
    filename: &str,
    rules: &ObfuscationRules,
) -> Result<ObfuscationOutput, ObfuscationError> {
    if !rules.enable_obfuscation {
        return Ok(ObfuscationOutput {
            code: source.to_string(),
            source_map: None,
        });
    }

    // --- Parse ---------------------------------------------------------------
    let alloc = Allocator::default();
    let source_type = SourceType::default().with_module(true);
    let ParserReturn {
        program, errors, ..
    } = Parser::new(&alloc, source, source_type).parse();

    if !errors.is_empty() {
        return Err(ObfuscationError::ParseError {
            file: filename.to_string(),
            message: errors[0].message.to_string(),
        });
    }

    // --- Collect top-level binding names ------------------------------------
    let reserved: FxHashSet<String> = rules
        .keep_global_names
        .iter()
        .chain(rules.keep_property_names.iter())
        .cloned()
        .collect();

    let mut binding_names: Vec<String> = Vec::new();

    for stmt in &program.body {
        match stmt {
            // var / let / const declarations
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    if let BindingPattern::BindingIdentifier(bi) = &declarator.id {
                        let name = bi.name.as_str().to_string();
                        if !reserved.contains(&name) && !binding_names.contains(&name) {
                            binding_names.push(name);
                        }
                    }
                }
            }
            // function declarations
            Statement::FunctionDeclaration(func) => {
                if let Some(id) = &func.id {
                    let name = id.name.as_str().to_string();
                    if !reserved.contains(&name) && !binding_names.contains(&name) {
                        binding_names.push(name);
                    }
                }
            }
            // class declarations
            Statement::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    let name = id.name.as_str().to_string();
                    if !reserved.contains(&name) && !binding_names.contains(&name) {
                        binding_names.push(name);
                    }
                }
            }
            _ => {}
        }
    }

    // --- Build rename map ----------------------------------------------------
    let mut name_alloc = NameAllocator::new(reserved);
    let mut rename_map: FxHashMap<String, String> = FxHashMap::default();
    for name in binding_names {
        rename_map.insert(name, name_alloc.next());
    }

    // --- Apply renames -------------------------------------------------------
    let code = if rename_map.is_empty() {
        source.to_string()
    } else {
        apply_renames(source, &rename_map)
    };

    // --- Strip console.log if requested -------------------------------------
    let code = if rules.remove_log {
        remove_console_log(&code)
    } else {
        code
    };

    Ok(ObfuscationOutput {
        code,
        source_map: None,
    })
}

/// Apply all renames from `rename_map` to `source` using whole-word, non-
/// property-access matching.
fn apply_renames(source: &str, rename_map: &FxHashMap<String, String>) -> String {
    // Collect (start, end, new_name) tuples for every occurrence.
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

    for (original, new_name) in rename_map {
        for (start, end) in find_identifier_occurrences(source, original) {
            replacements.push((start, end, new_name.clone()));
        }
    }

    if replacements.is_empty() {
        return source.to_string();
    }

    // Sort descending by start so we can splice without invalidating offsets.
    replacements.sort_by(|a, b| b.0.cmp(&a.0));
    // Remove duplicates at the same offset (keep one arbitrarily).
    replacements.dedup_by(|a, b| a.0 == b.0);

    let mut result = source.to_string();
    for (start, end, new_name) in replacements {
        if end <= result.len() {
            result.replace_range(start..end, &new_name);
        }
    }
    result
}

/// Return the `(start, end)` byte ranges of all whole-word, non-property-access
/// occurrences of `name` in `source`.
///
/// An occurrence is excluded when:
/// - the byte immediately before it is `.` (property access: `obj.name`)
/// - the byte immediately before or after it is an identifier continuation
///   character (`[A-Za-z0-9_$]`), which would make it a different identifier
fn find_identifier_occurrences(source: &str, name: &str) -> Vec<(usize, usize)> {
    let name_len = name.len();
    if name_len == 0 {
        return Vec::new();
    }

    let source_bytes = source.as_bytes();
    let mut result = Vec::new();
    let mut search_from = 0usize;

    while search_from + name_len <= source.len() {
        match source[search_from..].find(name) {
            None => break,
            Some(rel_pos) => {
                let abs_start = search_from + rel_pos;
                let abs_end = abs_start + name_len;

                // Boundary check: character before must not be an ident char or '.'
                let before_ok = abs_start == 0 || {
                    let prev = source_bytes[abs_start - 1];
                    !is_ident_char(prev) && prev != b'.'
                };

                // Boundary check: character after must not be an ident char
                let after_ok =
                    abs_end >= source.len() || !is_ident_char(source_bytes[abs_end]);

                if before_ok && after_ok {
                    result.push((abs_start, abs_end));
                }

                // Advance by 1 to catch adjacent non-overlapping matches.
                search_from = abs_start + 1;
            }
        }
    }
    result
}

/// Return `true` if `b` can be an interior byte of a JS identifier.
#[inline]
fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Remove lines that are bare `console.{log,warn,error,debug}(...)` calls.
///
/// This is a line-level heuristic sufficient for IIFE-wrapped bundles where
/// console calls typically occupy one line each.  A full AST-based removal
/// would be added in a follow-up if multi-line calls need to be handled.
fn remove_console_log(source: &str) -> String {
    let result: Vec<&str> = source
        .lines()
        .filter(|line| {
            let t = line.trim_start();
            !(t.starts_with("console.log(")
                || t.starts_with("console.warn(")
                || t.starts_with("console.error(")
                || t.starts_with("console.debug("))
        })
        .collect();
    result.join("\n")
}
