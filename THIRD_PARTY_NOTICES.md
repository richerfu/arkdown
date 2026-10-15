# Third-party licenses and source acknowledgements

Original arkdown contributions are licensed under `MIT OR Apache-2.0`, as
described in [LICENSE](LICENSE). Third-party code, embedded compiler libraries,
source-derived material and upstream notices retain their original terms.
The project license does not relicense those components.

## Linked compiler, bundler and signing dependencies

The following licenses and notices are copied unchanged from the Git revisions
pinned in `Cargo.toml` and `Cargo.lock`. Copyright notices in those files apply
to the corresponding third-party material.

| Project | Source revision | License | Included upstream files |
| --- | --- | --- | --- |
| [OXC (OpenHarmony fork)](https://github.com/ohos-rs/oxc) | `3f2b4b7d5338c10014c973e0db874ec2d3b655db` | MIT | [LICENSE](licenses/oxc/LICENSE), [THIRD-PARTY-LICENSE](licenses/oxc/THIRD-PARTY-LICENSE) |
| [Rolldown (OpenHarmony fork)](https://github.com/ohos-rs/rolldown) | `63112ceaa35eb24750a09d0ce3ae6f2f8074ed42` | MIT | [LICENSE](licenses/rolldown/LICENSE), [THIRD-PARTY-LICENSE](licenses/rolldown/THIRD-PARTY-LICENSE) |
| [tsrs](https://github.com/ohos-rs/tsrs) | `4d7e69df3847b5e0e5708b22a3f713082e5e6d02` | Apache-2.0 | [LICENSE](licenses/tsrs/LICENSE), [NOTICE](licenses/tsrs/NOTICE) |
| [hapsigner-rs](https://github.com/harmony-contrib/hapsigner-rs) | `0b4a16ad812d679c28e9d5602f0b0a239f28c719` | MIT | [LICENSE](licenses/hapsigner/LICENSE), [THIRD_PARTY_NOTICES.md](licenses/hapsigner/THIRD_PARTY_NOTICES.md) |

OXC's and Rolldown's upstream third-party notices include their own borrowed
material, including Rollup and esbuild. tsrs is a derivative work of TypeScript;
its complete upstream NOTICE also covers embedded declaration-library and
other third-party material. hapsigner's notices identify the OpenHarmony
signing sources and development signing assets. These upstream notices are
preserved in addition to each project's primary license.

NAPI-RS, mimalloc and the remaining Rust dependencies are identified by exact
version and license declaration in [the dependency inventory](licenses/dependencies.json).
That inventory includes unchanged license, copyright and attribution documents
from the resolved source packages, including vendored native libraries. Identical
documents share a content-addressed file under `licenses/texts/`; each inventory
entry identifies its original filename and source.

The inventory conservatively covers all non-workspace packages resolved by
`cargo metadata --locked`, including development, build, optional and platform
dependencies. It does not imply that every listed component is linked into
every native binary. For releases that provide only a license declaration,
the entry preserves that declaration and available attribution alongside the
standard terms of the selected license; these cases are explicitly recorded.

## OpenHarmony and TypeScript source references

Arkdown's build implementation is developed against source-observed OpenHarmony
behavior. The following repositories are acknowledged as source references;
source-derived metadata, fixtures and adapted material retain their upstream
copyright and license notices. This does not assert that the repositories are
redistributed wholesale.

| Source repository | Role | Included license and notices |
| --- | --- | --- |
| [developtools_ace_ets2bundle](https://gitee.com/openharmony/developtools_ace_ets2bundle) | ArkTS transformations, checker configuration, resolution and build-stage behavior; `tests/fixtures/compiler/ets-options.json` reflects its compiler options | [Apache-2.0](licenses/openharmony-ets2bundle/LICENSE) |
| [arkcompiler_ets_frontend](https://github.com/openharmony/arkcompiler_ets_frontend) | ABC compiler handoff and ArkGuard source behavior | [Apache-2.0](licenses/openharmony-arkcompiler-ets-frontend/LICENSE) |
| [third_party_typescript / OpenHarmony TypeScript fork](https://github.com/ohos-rs/TypeScript) | ArkTS 1.1 parser, checker and emitter source reference; not the current runtime checker | [LICENSE.txt](licenses/typescript/LICENSE.txt), [NOTICE.txt](licenses/typescript/NOTICE.txt) |
| [arkcompiler_runtime_core](https://gitee.com/openharmony/arkcompiler_runtime_core) | Bytecode layout and runtime source reference | [Apache-2.0](licenses/openharmony-runtime-core/LICENSE) |
| [interface_sdk-js](https://gitee.com/openharmony/interface_sdk-js) | SDK declarations, schemas and compiler metadata reference | [LICENSE](licenses/openharmony-sdk-js/LICENSE), [LICENCE.md](licenses/openharmony-sdk-js/LICENCE.md) |
| [interface_sdk_c](https://gitcode.com/openharmony/interface_sdk_c) | Native SDK and ABI source reference | [Apache-2.0](licenses/openharmony-sdk-c/LICENSE) |
| [commonlibrary_ets_utils](https://gitee.com/openharmony/commonlibrary_ets_utils) | ArkTS library source reference | [Apache-2.0](licenses/openharmony-ets-utils/LICENSE) |
| [manifest](https://gitee.com/openharmony/manifest) | OpenHarmony source inventory reference | [Apache-2.0](licenses/openharmony-manifest/LICENSE) |
| [developtools_hapsigner](https://github.com/openharmony/developtools_hapsigner) | Signing formats and OpenHarmony development signing assets | [Apache-2.0](licenses/openharmony-developtools-hapsigner/LICENSE) |
| [security_appverify](https://github.com/openharmony/security_appverify) | Signature-verification and code-sign format reference | [Apache-2.0](licenses/openharmony-security-appverify/LICENSE) |
| [developtools_packing_tool](https://github.com/openharmony/developtools_packing_tool) | OpenHarmony archive and packaging behavior | [Apache-2.0](licenses/openharmony-developtools-packing-tool/LICENSE) |
| [developtools_global_resource_tool](https://github.com/openharmony/developtools_global_resource_tool) | Resource-compiler behavior | [Apache-2.0](licenses/openharmony-developtools-global-resource-tool/LICENSE) |

The complete source revisions, original license URLs and SHA-256 fingerprints
are recorded in [licenses/repositories.json](licenses/repositories.json).
Git dependency revisions are the build pins. Other revisions identify the
source snapshot used to obtain the license text; they do not pin an installed SDK.

SDK executables and Hvigor used for local builds or parity verification are
provided by the user's SDK installation. Their licenses are not replaced by
arkdown's license. The external HmosWorld fixture is fetched separately as
described in `tests/fixtures/hmosworld/README.md`; its checkout is not included
in this repository's source or npm distribution.

## Distribution and maintenance

The root [NOTICE](NOTICE), license texts, this document and the `licenses/`
directory are included in the main npm package and every assembled native
platform package. The existing `tsrs-LICENSE.txt` and `tsrs-NOTICE.txt` names
remain available and are materialized from the same pinned upstream copies.

`pnpm --dir packages/arkdown licenses` materializes the canonical workspace
bundle. `prepack` and the `artifacts` script run the same writer. It verifies
the source fingerprints and rejects a stale dependency inventory when
`Cargo.lock` changes. Updating dependencies therefore requires refreshing the
inventory and preserving the license and notice texts from the new sources.
