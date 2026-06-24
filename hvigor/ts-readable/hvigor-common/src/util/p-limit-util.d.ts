/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/p-limit-util.d.ts
 */
export declare function createPromiseLimit<T>(concurrency: number): (task: () => Promise<T>) => Promise<T>;
