/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/hvigor-global-error-adaptor.d.ts
 */
import { HvigorErrorAdaptor } from './hvigor-error-adaptor';
/**
 * 适配一些来自hvigor和三方工具之外的错误，比如node本身的报错、未记录的报错
 */
export declare class HvigorGlobalErrorAdaptor extends HvigorErrorAdaptor {
    constructor(errorMsg: string);
    getErrorCodeJsonPath(): string[];
}
