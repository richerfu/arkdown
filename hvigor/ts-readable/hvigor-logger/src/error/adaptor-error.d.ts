/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor-error.d.ts
 */
export declare class AdaptorError extends Error {
    static readonly NAME = "AdaptorError";
    constructor(message?: string, stack?: string);
}
