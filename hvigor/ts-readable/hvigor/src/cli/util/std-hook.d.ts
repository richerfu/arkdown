/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/util/std-hook.d.ts
 */
export type UnHook = () => void;
export type Transformer = (output: string) => boolean;
export declare class StdHook {
    promise: Promise<void>;
    unhook: UnHook;
    constructor(p: Promise<void>, u: UnHook);
}
/**
 * 初始化stdout和stderr的hook
 */
export declare function initHook(): void;
/**
 * 取消stdout和stderr的hook
 */
export declare function unhook(): void;
