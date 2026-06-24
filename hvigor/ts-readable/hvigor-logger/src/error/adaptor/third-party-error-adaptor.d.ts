/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/third-party-error-adaptor.d.ts
 */
import { ErrorAdaptor, ErrorManagerAdaptor } from '../abstract-adaptor';
import { ThirdPartyError } from '../errors/third-party-error';
import { AdaptorErrorMessage, MatchFieldType, ToolErrorCode } from '../type';
/**
 * 三方工具的默认适配器
 */
export declare class ThirdPartyErrorAdaptor extends ErrorAdaptor implements ErrorManagerAdaptor {
    static readonly SOLUTIONS: string;
    protected _thirdPartyError: ThirdPartyError;
    protected readonly errorMessage: string | undefined;
    protected static readonly otherErrorCode: ToolErrorCode[];
    constructor(value: any, field?: MatchFieldType);
    /**
     * 解析三方工具的报错信息
     * @param {string} errorMessage 完整的错误信息
     */
    analyze(errorMessage: string): void;
    /**
     * 针对es2abc特殊报错格式进行调整
     */
    checkEs2abc(): void;
    getErrorMessage(): AdaptorErrorMessage[];
    getErrorCodeJsonPath(code?: string): string[];
}
