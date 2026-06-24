/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/mix-adaptor.d.ts
 */
import { ErrorAdaptor } from '../abstract-adaptor';
import { AdaptorErrorMessage } from '../type';
/**
 * 混合适配器
 * HvigorGlobalErrorAdaptor || ToolErrorAdaptor || PackToolAdaptor
 */
export declare class MixAdaptor implements ErrorAdaptor {
    private adaptor;
    constructor(errorMessage: string);
    getErrorMessage(): AdaptorErrorMessage[];
}
