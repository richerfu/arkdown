/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/util.d.ts
 */
export declare function getPackageName(id: string): string | null;
export declare function getMainFields(options: any): string[];
export declare function getPackageInfo(opts: any): Promise<any>;
export declare function normalizeInput(input: {
    [s: string]: unknown;
} | ArrayLike<unknown>): any[];
