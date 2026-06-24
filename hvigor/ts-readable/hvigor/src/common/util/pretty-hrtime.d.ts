/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/pretty-hrtime.d.ts
 */
interface Option {
    verbose?: boolean;
    precise?: boolean;
}
export default function (sourceArray: number[], options?: Option): string;
export {};
