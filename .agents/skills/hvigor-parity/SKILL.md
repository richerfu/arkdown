---
name: hvigor-parity
description: Mandatory hvigor source/cache/output comparison workflow for arkdown behavior changes. Use before editing resolver, transform, bundling, ABC, resource, packaging, signing, cache, CLI orchestration, or app runtime compatibility behavior.
---

# Hvigor Parity

## Rule

arkdown behavior must be derived from hvigor behavior. Do not guess, do not
patch symptoms with business fallbacks, and do not land minimum implementations.
If hvigor behavior cannot be matched for the declared scope, reject the
unsupported behavior with typed diagnostics and tests.

## Evidence Workflow

1. Identify the hvigor owner before editing production code:
   - `hvigor/`
   - `hvigor/ts-readable/`
   - SDK task/compiler code when hvigor delegates to SDK tooling
   - generated cache/output from a real hvigor build
2. For app-runtime failures, compare the smallest useful trio:
   - original source
   - hvigor-generated cache/output
   - arkdown-generated cache/output
3. Write down the observed parity contract in the code, test name, or test
   assertion when it is not obvious from the implementation.
4. Implement the behavior in the owning crate or stage object. Keep transform
   hot paths free of filesystem reads; feed them snapshots built during session
   or stage planning.
5. Verify with focused tests first, then broaden based on blast radius. For
   generated artifacts or app runtime fixes, rebuild the affected project and
   verify the generated output or device behavior.

## Prohibitions

- No guessed project-specific paths.
- No hardcoded package layouts, native declaration directories, cache paths, or
  app-specific package names unless they are explicit test fixtures.
- No string-only resolver implementations when hvigor uses package metadata,
  TypeScript resolved modules, lockfile data, SDK metadata, or task metadata.
- No fallback behavior that masks unsupported parity.
- No no-op adapters, placeholder cache keys, empty runners, or partial behavior
  described as temporary production support.
- No broad rewrites unless the hvigor owner and arkdown owning crate boundary
  are clear.

## Preferred Sources

- Resolver behavior: hvigor resolve plugins, TypeScript resolved-module data,
  OHPM manifests, lockfiles, and `oxc-resolve` or existing resolver crate APIs.
- ArkTS transform behavior: hvigor compiler plugin output and generated cache
  files, compared against OXC AST/codegen output.
- ABC behavior: upstream `es2abc`/ABC golden output, not bytecode guesses.
- Resource/package/sign behavior: hvigor task inputs, SDK tool command lines,
  and resulting archive/profile layouts.
