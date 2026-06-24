/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/log/arkts-base-log.d.ts
 */
import { ThirdPartyErrorOptions } from '@ohos/hvigor-logger';
import { HvigorRollupLogger } from './hvigor-rollup-logger';
type logType = string | number | unknown;
export interface LogEvent {
    time: Date;
    category: string;
    level: string;
    msg: string | number | unknown;
    type: string;
    fileId?: string;
}
export declare class LogCore {
    private _debugLog;
    private _infoLog;
    private _warnLog;
    private _errorLog;
    private _log;
    private readonly _category;
    private readonly _type;
    private readonly colorCodes;
    constructor(category: string, type: string);
    getDebugLogs(): LogEvent[];
    debug(...values: logType[]): void;
    getInfoLogs(): LogEvent[];
    info(...values: logType[]): void;
    getWarnLogs(): LogEvent[];
    warn(...values: logType[]): void;
    getErrorLogs(): LogEvent[];
    error(...values: logType[]): void;
    getLogs(): LogEvent[];
    clear(): void;
    private logMsg;
    private getLogEvent;
}
export declare class ArktsBaseLogger {
    protected static _logMap: Map<string, LogCore>;
    protected static _logMapRollup: Map<string, HvigorRollupLogger>;
    protected static _logEvents: LogEvent[];
    private static _logEventsHasError;
    private static sendLog;
    private static tsWatchCb;
    private static lastLogEvents;
    private static errorAdaptor;
    private static errorFileIds;
    static setLogCallback(sendLogCb: (logs: LogEvent[], lastReUsedFiles?: Set<string>) => void, tsWatchEndCb: () => void): void;
    static addLastLogEvents(logEvents: LogEvent[]): void;
    static getLastLogEvents(): LogEvent[];
    static getPluginLogger(category: string): LogCore;
    static getHvigorConsoleLogger(category: string): HvigorRollupLogger;
    static getHvigorLogger(category: string, type?: string): HvigorRollupLogger;
    static getLogger(category: string, type?: string): LogCore;
    static getLogMap(): Map<string, LogCore>;
    static addLogEvent(log: LogEvent): void;
    static getLogEvents(): LogEvent[];
    static checkLoggerHaveErrorInfo(): boolean;
    static sendLastLogEvents(lastReUsedFiles: Set<string>): void;
    static clearLogEvents(): void;
    static recordErrorFileId(fileId: string): void;
    static getErrorFileIds(): Set<string>;
    private static checkTsWatchEnd;
    static reset(): void;
    static clearLogMessages(): void;
    /**
     * 对三方工具暴露的接口
     * 使用方法：
     * 1. 传入参数type指定用于匹配错误信息的字段名，type有效值: 'code'、'checkMessage'、'id'
     * 2. 传入type对应的参数param（type -> param）: code -> code、checkMessage -> message、id -> id
     * 3. 如果type='none'或不存在, 需要将以下字段补齐：toolName、message、solutions
     * @param {ThirdPartyErrorOptions} options
     */
    static setError(options: ThirdPartyErrorOptions): void;
}
export {};
