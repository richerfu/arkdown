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
- [ ] Port static class fields through upstream static initializer functions;
  keep computed fields, accessors, and static blocks unsupported until their
  upstream lowering is ported.
- [x] Reject unsupported `with`, destructuring bindings/assignments, private
  names, JSX, V8 intrinsics, `import.meta`, and generator/yield lowering.
- [ ] Port full destructuring binding and assignment lowering.
- [ ] Port private field/private-in lowering with private environment support.
- [ ] Port computed class keys, accessors, private elements, and static blocks.
- [ ] Port generator and async-generator state-machine lowering.
- [ ] Port TypeScript runtime declarations beyond the existing module-record
  enum export metadata path.
- [ ] Broaden upstream bytecode parity coverage for for-of, try-finally,
  async/TLA, source-map/debug LNP, widgets, and HmosWorld cache-TS parity.
