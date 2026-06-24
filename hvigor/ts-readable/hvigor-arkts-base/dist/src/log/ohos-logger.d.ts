/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/log/ohos-logger.d.ts
 */
export type OhosLogger = {
    error(message: string, ...args: string[]): void;
    printErrorExit(errorId: string, messages?: string[], solutions?: string[][]): void;
};
export interface CompilerLogMsg {
    infoMsg?: string;
    warnMsg?: string;
    errorMsg: string;
}
export interface WatchCompilerMsg extends CompilerLogMsg {
    name: string;
}
export declare function addCompilerLogMsg(exceptionMsg: string): CompilerLogMsg;
