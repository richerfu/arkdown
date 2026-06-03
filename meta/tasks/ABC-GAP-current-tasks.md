# ABC Current Gap Closure Tasks

Scope: `arkdown-abc` lowering compared with upstream `es2abc` behavior on the
`feat/hmosworld-gap-closure` branch.

## Closure Rule

Unsupported constructs must not silently emit fallback bytecode. Each remaining
gap is either implemented with focused tests or rejected with
`AbcError::UnsupportedConstruct` until its upstream lowering is ported.

## Current Tasks

- [x] Replace silent statement fallthrough with explicit scanner coverage for
  nested function bodies and default exports.
- [x] Lower plain labels and labeled `break` / `continue` for `while`,
  `do...while`, `for`, `for-in`, and `for-of`.
- [x] Lower instance class fields through the upstream `CompileField` shape:
  initializer or `undefined`, followed by own-property definition.
- [x] Lower public non-computed static class fields through upstream static
  initializer functions.
- [x] Lower static blocks through upstream static initializer functions.
- [x] Lower class method accessors on the upstream `CompileMissingProperties`
  target (`classReg` for static, `protoReg` for instance).
- [x] Lower public static computed class fields through precomputed keys passed
  into upstream-shaped static initializer functions.
- [x] Lower public computed class methods/accessors through precomputed keys
  and upstream-shaped missing-property definitions.
- [x] Port public instance computed fields through class-evaluation key capture
  and constructor-time own-property definition by value.
- [ ] Port auto-accessor properties through their upstream class-environment
  lowering.
- [x] Reject unsupported `with`, private names, JSX, V8 intrinsics, and
  generator/yield lowering with explicit `UnsupportedConstruct` errors.
- [x] Lower `import.meta` to `getimportmeta` (0xd5) per upstream es2abc ISA.
- [x] Erase `AnnotationDeclaration` (@interface) as metadata-only ArkTS node.
- [x] Port full destructuring binding and assignment lowering (array via iterator
  protocol, object via property loads, defaults via undefined check, rest,
  nested patterns, for-of left-side, function parameters, assignment forms).
- [x] Port auto-accessor properties: synthesized private backing field slot,
  getter/setter MethodRecords, DefineGetterSetterByValue (0xbc) installation.
- [x] Port private field/private-in lowering: LdPrivateProperty (0xd8),
  StPrivateProperty (0xd9), TestIn (0xda),
  CallRuntimeCreatePrivateProperty (0xfb04),
  CallRuntimeDefinePrivateProperty (0xfb05); private slot map threaded
  through class method compilation; private backing slots for AccessorProperty.
- [x] `with` statement: rejection IS the correct upstream behavior — es2panda has
  no `withStatement.cpp` in ir/statements; `with` is a SyntaxError in strict
  mode (all ArkTS modules are strict).
- [x] Port generator and async-generator state-machine lowering:
  CreateGeneratorObj + initial SuspendGenerator/ResumeGenerator
  prologue; emit_yield_expr with GetResumeMode for return/throw/next
  mode-checking; compile_generator_body route in compile_function_record.
- [x] Implement top-level await (TLA): detect top-level await in module
  body and emit asyncfunctionenter prologue.
- [x] Lower TypeScript enum runtime declarations with upstream transformer
  semantics for name/value and numeric reverse mappings.
- [x] Erase `declare` TypeScript module/namespace declarations as type-only.
- [x] Erase `import type Alias = ...` TypeScript import-equals declarations as
  type-only, including external module references without runtime module
  requests.
- [x] Lower runtime TypeScript import-equals declarations through upstream
  transformer-shaped local bindings for `require("...")` and qualified member
  aliases, while erasing aliases used only in type positions.
- [x] Erase TypeScript `export as namespace ...` declarations as declaration
  file/type-only syntax.
- [x] Erase TypeScript `declare` class/var/enum declarations as type-only
  declarations, including exported ambient classes.
- [x] Lower non-declare TypeScript namespace/module runtime declarations through
  the TypeScript transformer's upstream-shaped namespace IIFE form.
- [x] Lower CommonJS TypeScript `export =` assignments through the TypeScript
  transformer's `module.exports = ...` form, while reporting ESM diagnostics.
- [x] Verify TypeScript runtime declaration edge cases: nested namespaces,
  const enum (inlined by OXC transformer), module augmentation erasure.
- [ ] Broaden upstream bytecode parity coverage for for-of, try-finally,
  async/TLA, source-map/debug LNP, widgets, and HmosWorld cache-TS parity.
