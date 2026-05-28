# Upstream Bytecode Fixtures

These fixtures are migrated from OpenHarmony `arkcompiler_ets_frontend` tests.
Keep paths below aligned with the upstream repository layout so additional cases
can be copied by category.

- `es2panda/bytecode/js/lexicalEnv/arrow-function-reference-ctx`
- `es2panda/bytecode/commonjs/test-commonjs-main-func-args`
- `es2panda/bytecode/js/class/test-explicit-constructor`

The `*-expected.txt` files preserve the upstream bytecode expectations currently
used by Rust parity tests. Some tests are marked `#[ignore]` until the
corresponding lowering pass is ported.
