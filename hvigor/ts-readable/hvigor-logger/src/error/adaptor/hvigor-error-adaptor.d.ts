/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/hvigor-error-adaptor.d.ts
 */
import { ErrorAdaptor, ErrorManagerAdaptor } from '../abstract-adaptor';
import { HvigorError } from '../errors/hvigor-error';
import { AdaptorErrorMessage, MatchFieldType, ToolErrorCode } from '../type';
/**
 * hvigor的适配器
 */
export declare class HvigorErrorAdaptor extends ErrorAdaptor implements ErrorManagerAdaptor {
    protected _hvigorError: HvigorError;
    constructor(value: any, field?: MatchFieldType);
    getErrorMessage(): AdaptorErrorMessage[];
    getErrorCodeJsonPath(): string[];
    getToolErrorCode(): ToolErrorCode;
    formatMessage(...args: unknown[]): this;
    formatSolutions(index: number, ...args: unknown[]): this;
    isMatchSuccess(): boolean;
}
