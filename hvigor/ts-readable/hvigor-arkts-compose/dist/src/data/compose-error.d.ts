/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/data/compose-error.d.ts
 */
export declare const ARKTS_COMPILER_ERROR_NAME = "ArkTSCompilerError";
export declare class ArkTsCompilerError extends Error {
    readonly stack: string | undefined;
    constructor(message: string, stack?: string);
}
export declare function throwArkTsCompilerError(...message: string[]): void;
export interface ExtraInfo {
    infoMsg?: string;
    warnMsg?: string;
}
/**
 * 抛出去的错误对象 + 额外信息
 */
export declare class ExtendedErrorInfo extends Error {
    readonly extraInfo: ExtraInfo | undefined;
    constructor(message: string, extraInfo?: ExtraInfo);
}
