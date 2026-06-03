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
- [ ] Port instance computed fields and auto-accessor properties through their
  upstream class-environment lowering.
- [x] Reject unsupported `with`, destructuring bindings/assignments, private
  names, JSX, V8 intrinsics, `import.meta`, and generator/yield lowering.
- [ ] Port full destructuring binding and assignment lowering.
- [ ] Port private field/private-in lowering with private environment support.
- [ ] Port remaining computed class keys, auto-accessor properties, and private
  elements.
- [ ] Port generator and async-generator state-machine lowering.
- [x] Lower TypeScript enum runtime declarations with upstream transformer
  semantics for name/value and numeric reverse mappings.
- [x] Erase `declare` TypeScript module/namespace declarations as type-only.
- [x] Erase `import type Alias = ...` TypeScript import-equals declarations as
  type-only, including external module references without runtime module
  requests.
- [x] Lower runtime TypeScript import-equals declarations through upstream
  transformer-shaped local bindings for `require("...")` and qualified member
  aliases, while erasing aliases used only in type positions.
- [ ] Port remaining TypeScript runtime declarations beyond enum lowering,
  including non-declare namespace/module lowering.
- [ ] Broaden upstream bytecode parity coverage for for-of, try-finally,
  async/TLA, source-map/debug LNP, widgets, and HmosWorld cache-TS parity.
