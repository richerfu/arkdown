/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/submit-option.d.ts
 */
import { Priority } from '../enum/priority';
/**
 * 提交选项
 *
 * @since 2022/8/23
 */
export type SubmitOption = {
    workInput?: unknown;
    callback?: Function;
    callbackInput?: unknown[];
    priority?: Priority;
    targetWorkers?: number[];
    useReturnVal?: boolean;
    hasSideEffects?: boolean;
    preludeDeps?: string[];
    memorySensitive?: boolean;
};
