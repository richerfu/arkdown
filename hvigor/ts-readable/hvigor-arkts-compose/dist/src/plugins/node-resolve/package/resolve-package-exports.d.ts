/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/package/resolve-package-exports.d.ts
 */
export declare function resolvePackageExports(context: {
    importSpecifier: any;
    importer: any;
    pkgJsonPath: any;
}, subPath: string, exports: any): Promise<string>;
