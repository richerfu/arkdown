/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/errors/third-party-error.d.ts
 */
import { BaseError } from '../abstract-error';
import { MatchOptions, ThirdPartyErrorInfo } from '../type';
/**
 * 三方组价的报错
 */
export declare class ThirdPartyError extends BaseError<ThirdPartyErrorInfo> {
    protected _toolName: string | undefined;
    constructor(errorJsonPaths: string[], matchOptions: MatchOptions);
    get toolName(): string | undefined;
    set toolName(toolName: string | undefined);
}
