/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/adaptor-factory.d.ts
 */
import { ErrorAdaptor } from '../abstract-adaptor';
/**
 * 创建错误信息适配器的简单工程类
 */
export declare class AdaptorFactory {
    static createErrorAdaptor(errorMessage: string): ErrorAdaptor;
    static isThirdPartyError(errorMessage: string): boolean;
    static hasErrorCode(errorMessage: string): boolean;
}
