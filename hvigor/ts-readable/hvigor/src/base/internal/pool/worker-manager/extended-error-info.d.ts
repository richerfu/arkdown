/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-manager/extended-error-info.d.ts
 */
export interface ExtraInfo {
    infoMsg?: string;
    warnMsg?: string;
}
/**
 * 抛出去的错误对象 + 额外信息
 */
export declare class ExtendedErrorInfo extends Error {
    extraInfo: ExtraInfo | undefined;
    constructor(message?: string, extraInfo?: ExtraInfo, stack?: string);
}
