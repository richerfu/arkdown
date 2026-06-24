/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/options.d.ts
 */
export type Option = {
    [index: string]: unknown;
};
export interface Named extends Option {
    name?: string;
}
export interface RequiredNamed extends Named {
    name: string;
}
export interface Configurable extends Option {
    runtimeOS?: string;
}
