/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/log.d.ts
 */
import { HvigorRollupLogger } from '@ohos/hvigor-arkts-base';
/**
 * 对应Hvigor-Ohos-Plugin的定制logger
 */
export type OhosLogger = {
    error(message: unknown, ...args: unknown[]): void;
    _buildError(cause: string, errorCode?: string): OhosLogger;
    _printErrorAndExit(moduleName?: string): void;
    warn(message: unknown, ...args: unknown[]): void;
    printErrorExit(errorId: string, messages?: unknown[], solutions?: unknown[][]): void;
};
/**
 * 抽出函数，调用HvigorRollupLogger的printError函数
 * @param _log
 * @param errorId
 * @param messages
 * @param solutions
 */
export declare function hvigorConsoleLoggerPrintError(_log: HvigorRollupLogger, errorId: string, messages?: unknown[], solutions?: unknown[][]): void;
export declare function hvigorConsoleLoggerPrintErrorAndExit(_log: HvigorRollupLogger, errorId: string, messages?: unknown[], solutions?: unknown[][]): void;
