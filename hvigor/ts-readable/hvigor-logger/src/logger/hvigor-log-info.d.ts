/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-log-info.d.ts
 */
import { LogLevel } from '../consts/errorlog-consts';
import { THvigorLogInfo } from '../types/error-log-types';
/**
 * 对接打印框架的类
 */
export declare class HvigorLogInfo {
    private message;
    private logLevel;
    constructor(logInfo?: THvigorLogInfo);
    setMessage(message: string): HvigorLogInfo;
    setLogLevel(level: LogLevel): HvigorLogInfo;
    getMessage(): string;
    getLogLevel(): LogLevel;
}
