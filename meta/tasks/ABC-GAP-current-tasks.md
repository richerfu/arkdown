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
  `do...while`, and `for`.
- [x] Preserve initialized instance class fields; reject static, computed,
  uninitialized, accessor, and static-block class fields until upstream static
  initialization and computed-key lowering are ported.
- [x] Reject unsupported `with`, destructuring bindings/assignments, private
  names, JSX, V8 intrinsics, `import.meta`, and generator/yield lowering.
- [ ] Port full destructuring binding and assignment lowering.
- [ ] Port private field/private-in lowering with private environment support.
- [ ] Port static class fields, computed class keys, accessors, and static
  blocks.
- [ ] Port generator and async-generator state-machine lowering.
- [ ] Port TypeScript runtime declarations beyond the existing module-record
  enum export metadata path.
- [ ] Broaden upstream bytecode parity coverage for for-of, try-finally,
  async/TLA, source-map/debug LNP, widgets, and HmosWorld cache-TS parity.
