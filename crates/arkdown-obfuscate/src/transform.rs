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
use oxc_ast::ast::{BindingPattern, Expression, Statement};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::{GetSpan, SourceType, Span};
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

impl ObfuscationRules {
    /// Apply these obfuscation rules to `source` text.
    ///
    /// Returns the original source unchanged when neither
    /// `enable_obfuscation` nor `compact` is set.
    pub fn apply(
        &self,
        source: &str,
        filename: &str,
    ) -> Result<ObfuscationOutput, ObfuscationError> {
        if !self.enable_obfuscation && !self.compact {
            return Ok(ObfuscationOutput {
                code: source.to_string(),
                source_map: None,
            });
        }

        // --- Identifier renaming (only when -enable-obfuscation is set) ----------
        let code = if self.enable_obfuscation {
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

            let reserved: FxHashSet<String> = self
                .keep_global_names
                .iter()
                .chain(self.keep_property_names.iter())
                .cloned()
                .collect();

            let mut binding_names: Vec<String> = Vec::new();

            for stmt in &program.body {
                match stmt {
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
                    Statement::FunctionDeclaration(func) => {
                        if let Some(id) = &func.id {
                            let name = id.name.as_str().to_string();
                            if !reserved.contains(&name) && !binding_names.contains(&name) {
                                binding_names.push(name);
                            }
                        }
                    }
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

            let mut name_alloc = NameAllocator::new(reserved);
            let mut rename_map: FxHashMap<String, String> = FxHashMap::default();
            for name in binding_names {
                rename_map.insert(name, name_alloc.next());
            }

            let renamed = if rename_map.is_empty() {
                source.to_string()
            } else {
                Self::apply_renames(source, &rename_map)
            };

            if self.remove_log {
                Self::remove_console_log(&renamed)
            } else {
                renamed
            }
        } else {
            source.to_string()
        };

        // --- Minify if -compact is requested ------------------------------------
        let code = if self.compact {
            crate::minifier::minify_source(&code, filename)?
        } else {
            code
        };

        Ok(ObfuscationOutput {
            code,
            source_map: None,
        })
    }
}

impl ObfuscationRules {
    /// Apply all renames from `rename_map` to `source` using whole-word, non-
    /// property-access matching.
    fn apply_renames(source: &str, rename_map: &FxHashMap<String, String>) -> String {
        // Collect (start, end, new_name) tuples for every occurrence.
        let mut replacements: Vec<(usize, usize, String)> = Vec::new();

        for (original, new_name) in rename_map {
            for (start, end) in Self::find_identifier_occurrences(source, original) {
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
                        !Self::is_ident_char(prev) && prev != b'.'
                    };

                    // Boundary check: character after must not be an ident char
                    let after_ok =
                        abs_end >= source.len() || !Self::is_ident_char(source_bytes[abs_end]);

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

    /// Remove expression statements that are bare `console.{log,warn,error,debug}(...)` calls.
    fn remove_console_log(source: &str) -> String {
        let alloc = Allocator::default();
        let parsed = Parser::new(&alloc, source, SourceType::default().with_module(true)).parse();
        if parsed.panicked || !parsed.errors.is_empty() {
            return source.to_owned();
        }

        let patches = parsed
            .program
            .body
            .iter()
            .filter(Self::is_console_log_statement)
            .map(|statement| SourcePatch::removal(source, statement.span()))
            .collect::<Vec<_>>();
        SourcePatch::apply_all(source, patches)
    }

    fn is_console_log_statement(statement: &&Statement<'_>) -> bool {
        let Statement::ExpressionStatement(statement) = statement else {
            return false;
        };
        let Expression::CallExpression(call) = &statement.expression else {
            return false;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return false;
        };
        matches!(&member.object, Expression::Identifier(identifier) if identifier.name.as_str() == "console")
            && matches!(
                member.property.name.as_str(),
                "log" | "warn" | "error" | "debug"
            )
    }
}

#[derive(Debug)]
struct SourcePatch {
    start: usize,
    end: usize,
    replacement: String,
}

impl SourcePatch {
    fn removal(source: &str, span: Span) -> Self {
        let mut end = span.end as usize;
        if source
            .as_bytes()
            .get(end)
            .is_some_and(|byte| *byte == b'\n')
        {
            end += 1;
        }
        Self {
            start: span.start as usize,
            end,
            replacement: String::new(),
        }
    }

    fn apply_all(source: &str, mut patches: Vec<Self>) -> String {
        if patches.is_empty() {
            return source.to_owned();
        }
        patches.sort_by_key(|patch| patch.start);

        let mut out = String::with_capacity(source.len());
        let mut last_end = 0usize;
        for patch in patches {
            if patch.start < last_end {
                continue;
            }
            out.push_str(&source[last_end..patch.start]);
            out.push_str(&patch.replacement);
            last_end = patch.end;
        }
        out.push_str(&source[last_end..]);
        out
    }
}
