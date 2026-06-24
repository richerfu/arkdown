# hvigor Reconstruction Workspace

This directory contains local reconstruction artifacts for DevEco Studio
6.1.1.280 hvigor analysis and the later Rust rewrite plan.

## Human-Readable TypeScript

The source-reading layer is:

```text
hvigor/ts-readable/
```

It is generated directly from the local DevEco installation at:

```text
/Users/ranger/.meat/ide/DevEco-Studio-6.1.1.280.app/Contents/tools/hvigor
```

The generator is:

```text
hvigor/tools/reconstruct-readable-ts.cjs
```

Included first-party packages:

| Package | Version | TS files | d.ts files |
| --- | ---: | ---: | ---: |
| `hvigor` | 6.24.2 | 292 | 292 |
| `hvigor-ohos-plugin` | 6.24.2 | 496 | 496 |
| `hvigor-arkts-base` | 6.24.2 | 30 | 30 |
| `hvigor-arkts-compose` | 6.24.2 | 73 | 73 |
| `hvigor-common` | 6.24.2 | 22 | 22 |
| `hvigor-logger` | 6.24.2 | 58 | 58 |

Third-party dependencies are intentionally excluded. The output is a readable
TypeScript reconstruction from minified CommonJS JavaScript plus adjacent
declaration files. It is a reference baseline for behavior analysis; Rust ports
must follow arkdown crate ownership and typed-plan boundaries rather than
preserving the CommonJS runtime shape.

The reconstruction removes the major CommonJS/TypeScript compiler glue:

- `Object.defineProperty(exports, "__esModule", ...)`
- `__importDefault`, `__importStar`, `__createBinding`, `__exportStar`
- top-level `require(...)` import declarations
- top-level `exports.foo = ...` assignments
- `!0`, `!1`, and `void 0`

The output keeps dynamic `require()` calls when they represent runtime behavior,
such as SDK plugin loading, daemon module loading, package.json version reads,
or user hvigorfile loading.

Regenerate:

```bash
node hvigor/tools/reconstruct-readable-ts.cjs
```

The readable generation manifest is:

```text
hvigor/manifests/ts-readable-reconstruction.json
```

## Flow Documents

- [flow.md](flow.md): end-to-end hvigor build flow and Mermaid diagrams.
