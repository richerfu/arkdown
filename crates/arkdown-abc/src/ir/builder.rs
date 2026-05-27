use oxc_allocator::Allocator;
use oxc_ast::ast::{FunctionBody, Program, Statement};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use crate::emit::record::{ClassRecord, MethodRecord};
use crate::emit::strings::StringTable;
use crate::error::AbcError;
use crate::ir::opcodes::Insn;

/// Parse `source` and build a [`ClassRecord`] named `record_name`.
///
/// This is the main entry point from [`crate::compiler::AbcCompiler`].
/// The function:
///
/// 1. Parses the source with `oxc_parser` (treating it as an MJS module).
/// 2. Returns [`AbcError::Parse`] if there are any hard parse errors.
/// 3. Emits a `main` method that represents the module's top-level IIFE body.
/// 4. Emits additional methods for every named top-level function declaration.
///
/// ## Stub note
///
/// Real body compilation (statement → opcode lowering) is a TODO.  The
/// generated methods currently emit only `ReturnUndefined`, which produces a
/// structurally valid `.abc` that can be loaded by ArkRuntime without crashing;
/// it simply does not execute user code.  Full code-generation is tracked in
/// SPEC-009 follow-up tasks.
pub fn build_record(
    source: &str,
    record_name: &str,
    strings: &mut StringTable,
) -> Result<ClassRecord, AbcError> {
    let allocator = Allocator::default();
    // Parse as TSX so that TypeScript type annotations (`: Want`,
    // `): void`, etc.) produced by the codegen fallback in the loader are
    // accepted without errors.  TSX is a superset of plain JS, so purely
    // JavaScript IIFE-wrapped chunks also parse without issue.
    let source_type = SourceType::tsx();
    let ParserReturn { program, panicked, errors, .. } =
        Parser::new(&allocator, source, source_type).parse();

    // A hard parser panic (not recoverable parse errors) is the only case
    // we treat as fatal.  Recoverable errors are tolerated: the ABC
    // compiler currently emits `ReturnUndefined` stubs for all methods
    // regardless, so a partial AST is sufficient for record generation.
    if panicked {
        return Err(AbcError::Parse {
            file: record_name.to_string(),
            message: errors
                .first()
                .map(|e| e.message.to_string())
                .unwrap_or_else(|| "parser panic".to_string()),
        });
    }

    let name_idx = strings.intern(record_name);
    let mut methods: Vec<MethodRecord> = Vec::new();

    // Build a `main` method that represents the IIFE execution entry point.
    let main_idx = strings.intern("main");
    let bytecode = compile_program_body(&program, strings);
    methods.push(MethodRecord {
        name_idx: main_idx,
        bytecode,
        num_vregs: 8, // conservative fixed estimate; TODO: derive from register allocator
        num_args: 0,
    });

    // Emit a method stub for each named top-level function declaration so that
    // the ArkRuntime class-linker can bind them.
    for stmt in &program.body {
        if let Statement::FunctionDeclaration(func) = stmt {
            if let Some(id) = &func.id {
                let func_name_idx = strings.intern(id.name.as_str());
                let func_bytecode =
                    compile_function_body(func.body.as_deref(), strings);
                methods.push(MethodRecord {
                    name_idx: func_name_idx,
                    bytecode: func_bytecode,
                    num_vregs: 8,
                    num_args: func.params.items.len() as u16,
                });
            }
        }
    }

    Ok(ClassRecord { name_idx, methods })
}

/// Compile the top-level body of a parsed program into bytecode.
///
/// TODO: Lower each [`Statement`] in `program.body` into a sequence of
/// [`Insn`]s using a recursive visitor.  For now we emit a single
/// `ReturnUndefined` so that the generated `.abc` is structurally valid.
fn compile_program_body(
    _program: &Program<'_>,
    strings: &mut StringTable,
) -> Vec<u8> {
    Insn::ReturnUndefined.encode(strings)
}

/// Compile a function body into bytecode.
///
/// TODO: Lower [`FunctionBody::statements`] into [`Insn`] sequences.
/// For now emits `ReturnUndefined` as a safe placeholder.
fn compile_function_body(
    _body: Option<&FunctionBody<'_>>,
    strings: &mut StringTable,
) -> Vec<u8> {
    Insn::ReturnUndefined.encode(strings)
}
