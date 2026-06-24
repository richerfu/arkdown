/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/errors/hvigor-error.d.ts
 */
import { BaseError } from '../abstract-error';
import { HvigorErrorInfo, MatchOptions } from '../type';
/**
 * hvigor的报错
 */
export declare class HvigorError extends BaseError<HvigorErrorInfo> {
    private _originMessage;
    private _originSolutions;
    constructor(errorJsonPaths: string[], matchOptions: MatchOptions);
    formatMessage(...args: unknown[]): void;
    formatSolutions(index: number, ...args: unknown[]): void;
    get originMessage(): string | undefined;
    get originSolutions(): string[] | undefined;
}
