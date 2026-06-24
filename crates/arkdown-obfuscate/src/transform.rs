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
//! 4. Mutate matching OXC binding and identifier-reference nodes in place.
//! 5. Print the transformed AST through OXC codegen.
//!
//! This approach keeps OHOS `keep_global_name` semantics separate from the
//! generic `oxc_minifier` mangler while leaving source text reconstruction to
//! OXC codegen.

use oxc_allocator::{Allocator, Vec as OxcVec};
use oxc_ast::ast::{
    BindingIdentifier, BindingPattern, Expression, IdentifierReference, Program, Statement,
};
use oxc_ast_visit::VisitMut;
use oxc_codegen::Codegen;
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
            let source_type = SourceType::from_path(filename)
                .unwrap_or_else(|_| SourceType::default())
                .with_module(true);
            let ParserReturn {
                mut program,
                diagnostics,
                ..
            } = Parser::new(&alloc, source, source_type).parse();

            if !diagnostics.is_empty() {
                return Err(ObfuscationError::ParseError {
                    file: filename.to_string(),
                    message: diagnostics[0].message.to_string(),
                });
            }

            let reserved: FxHashSet<String> = self
                .keep_global_names
                .iter()
                .chain(self.keep_property_names.iter())
                .cloned()
                .collect();

            let binding_names = TopLevelBindingCollector::new(&program, &reserved).collect();

            let mut name_alloc = NameAllocator::new(reserved);
            let mut rename_map: FxHashMap<String, String> = FxHashMap::default();
            for name in binding_names {
                rename_map.insert(name, name_alloc.next());
            }

            let mut changed = false;
            if !rename_map.is_empty() {
                RenameAstMutator::new(&alloc, &rename_map).visit_program(&mut program);
                changed = true;
            }
            if self.remove_log && Self::remove_console_log_statements(&alloc, &mut program) {
                changed = true;
            }
            if changed {
                Codegen::new().build(&program).code
            } else {
                source.to_string()
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

struct TopLevelBindingCollector<'a, 'r> {
    program: &'a Program<'a>,
    reserved: &'r FxHashSet<String>,
    seen: FxHashSet<String>,
    names: Vec<String>,
}

impl<'a, 'r> TopLevelBindingCollector<'a, 'r> {
    fn new(program: &'a Program<'a>, reserved: &'r FxHashSet<String>) -> Self {
        Self {
            program,
            reserved,
            seen: FxHashSet::default(),
            names: Vec::new(),
        }
    }

    fn collect(mut self) -> Vec<String> {
        for statement in &self.program.body {
            self.collect_statement(statement);
        }
        self.names
    }

    fn collect_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
                        self.push_name(binding.name.as_str());
                    }
                }
            }
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    self.push_name(id.name.as_str());
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    self.push_name(id.name.as_str());
                }
            }
            _ => {}
        }
    }

    fn push_name(&mut self, name: &str) {
        if self.reserved.contains(name) || !self.seen.insert(name.to_owned()) {
            return;
        }
        self.names.push(name.to_owned());
    }
}

impl ObfuscationRules {
    /// Remove expression statements that are bare `console.{log,warn,error,debug}(...)` calls.
    fn remove_console_log_statements<'a>(
        allocator: &'a Allocator,
        program: &mut Program<'a>,
    ) -> bool {
        let mut changed = false;
        let mut body = OxcVec::new_in(allocator);
        let old_body = std::mem::replace(&mut program.body, OxcVec::new_in(allocator));
        for statement in old_body {
            if ConsoleLogStatement::new(&statement).matches() {
                changed = true;
            } else {
                body.push(statement);
            }
        }
        program.body = body;
        changed
    }
}

struct ConsoleLogStatement<'a> {
    statement: &'a Statement<'a>,
}

impl<'a> ConsoleLogStatement<'a> {
    fn new(statement: &'a Statement<'a>) -> Self {
        Self { statement }
    }

    fn matches(&self) -> bool {
        let Statement::ExpressionStatement(statement) = self.statement else {
            return false;
        };
        let Expression::CallExpression(call) = &statement.expression else {
            return false;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return false;
        };
        ConsoleCallCallee::new(&member.object, member.property.name.as_str()).matches()
    }
}

struct ConsoleCallCallee<'a> {
    object: &'a Expression<'a>,
    property: &'a str,
}

impl<'a> ConsoleCallCallee<'a> {
    fn new(object: &'a Expression<'a>, property: &'a str) -> Self {
        Self { object, property }
    }

    fn matches(&self) -> bool {
        self.has_console_object() && ConsoleMethodName::new(self.property).is_removable()
    }

    fn has_console_object(&self) -> bool {
        matches!(self.object, Expression::Identifier(identifier) if identifier.name.as_str() == ConsoleRuntimeName::CONSOLE)
    }
}

struct ConsoleRuntimeName;

impl ConsoleRuntimeName {
    const CONSOLE: &'static str = "console";
}

struct ConsoleMethodName<'a> {
    name: &'a str,
}

impl<'a> ConsoleMethodName<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn is_removable(&self) -> bool {
        matches!(
            self.name,
            Self::LOG | Self::WARN | Self::ERROR | Self::DEBUG
        )
    }

    const LOG: &'static str = "log";
    const WARN: &'static str = "warn";
    const ERROR: &'static str = "error";
    const DEBUG: &'static str = "debug";
}

struct RenameAstMutator<'a> {
    allocator: &'a Allocator,
    rename_map: &'a FxHashMap<String, String>,
}

impl<'a> RenameAstMutator<'a> {
    fn new(allocator: &'a Allocator, rename_map: &'a FxHashMap<String, String>) -> Self {
        Self {
            allocator,
            rename_map,
        }
    }

    fn renamed_name(&self, name: &str) -> Option<&'a str> {
        let Some(replacement) = self.rename_map.get(name) else {
            return None;
        };
        Some(self.allocator.alloc_str(replacement))
    }
}

impl<'a> VisitMut<'a> for RenameAstMutator<'a> {
    fn visit_binding_identifier(&mut self, ident: &mut BindingIdentifier<'a>) {
        if let Some(replacement) = self.renamed_name(ident.name.as_str()) {
            ident.name = replacement.into();
        }
    }

    fn visit_identifier_reference(&mut self, ident: &mut IdentifierReference<'a>) {
        if let Some(replacement) = self.renamed_name(ident.name.as_str()) {
            ident.name = replacement.into();
        }
    }
}
