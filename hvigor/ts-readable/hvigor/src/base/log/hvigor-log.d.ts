/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/log/hvigor-log.d.ts
 */
import { AdaptorErrorMessage, HvigorErrorAdaptor } from '@ohos/hvigor-logger';
import type { Configuration, Level, Logger } from 'log4js';
import { MetricLogType } from '../metrics/event/log-event';
import { FileLogger } from './adaptor/file-logger';
/**
 * 基于log4js封装的HvigorLogger
 *
 * @since 2022/03/02
 */
export declare class HvigorLogger {
    private static instanceMap;
    protected _logger: Logger;
    protected readonly _filelogger: Logger;
    protected readonly anonymizeFileLogger: FileLogger;
    protected durationId: string | undefined;
    protected constructor(category?: string);
    protected static getInstance<T>(logger: any, category?: string): T;
    /**
     * 获取对于类别的HvigorLogger实例
     *
     * @param {string} category 默认是default
     * @return {HvigorLogger}
     */
    static getLogger(category?: string): HvigorLogger;
    static getLoggerWithDurationId(category: string, durationId: string): HvigorLogger;
    static clean(): void;
    log(level: Level | string, ...args: unknown[]): void;
    debug(message: unknown, ...args: unknown[]): void;
    info(message: unknown, ...args: unknown[]): void;
    warn(message: unknown, ...args: unknown[]): void;
    error(message: unknown, ...args: unknown[]): void;
    anonymizeDebug(message: unknown, ...args: unknown[]): void;
    _printTaskExecuteInfo(taskPath: string, time: string): void;
    _printFailedTaskInfo(taskPath: string): void;
    _printDisabledTaskInfo(taskPath: string): void;
    _printUpToDateTaskInfo(taskPath: string): void;
    _printStackErrorToFile(message: unknown, ...args: unknown[]): void;
    errorMessageExit(message: string, ...args: unknown[]): void;
    errorExit(e: Error, message?: string, ...args: unknown[]): void;
    getLevel(): Level | string;
    setLevel(level: Level | string): void;
    createLogEventByDurationId(message: unknown, logType: MetricLogType, ...args: unknown[]): unknown;
    getMessage(message: string, ...args: unknown[]): string;
    protected getAdaptor(errorId: string): HvigorErrorAdaptor;
    /**
     * 将适配器返回的里的错误信息转成最终打印的错误信息
     * @param {AdaptorErrorMessage} adaptorErrorMessage
     * @returns {string}
     * @protected
     */
    protected combinePhase(adaptorErrorMessage: AdaptorErrorMessage): string;
    protected formatErrorAdaptor(errorId: string, messages?: unknown[], solutions?: unknown[][]): HvigorErrorAdaptor;
    /**
     * 三段式打印错误信息(不阻塞构建)
     * @param adaptorErrorMessages AdaptorErrorMessage的数组
     * @param trySuggestion
     */
    printErrorWithAdaptorErrorMessage(adaptorErrorMessages: AdaptorErrorMessage[], trySuggestion?: string): void;
    /**
     * 三段式打印错误信息(不阻塞构建)
     * @param errorId 错误码表文件(如：hvigor.json)中的key
     * @param messages message格式化字符串
     * @param solutions solutions格式化字符串，二维数组，每一行对应solution中的一行
     */
    printError(errorId: string, messages?: unknown[], solutions?: unknown[][]): void;
    /**
     * 三段式打印错误信息(阻塞构建)
     * @param errorId 错误码表文件(如：hvigor.json)中的key
     * @param messages message格式化字符串
     * @param solutions solutions格式化字符串，二维数组，每一行对应solution中的一行
     * @param stack stack最终打印的堆栈
     */
    printErrorExit(errorId: string, messages?: unknown[], solutions?: unknown[][], stack?: string): void;
    /**
     * 在 hvigorfile.ts 中调用的函数如果使用 printErrorExit 报错退出会打印 throw 那一行的代码，而压缩后的代码只有一行，会把整个文件的代码都打印出来
     * 所以新增此方法避免打印多余信息影响阅读
     */
    printErrorExitWithoutStack(errorId: string, messages?: unknown[], solutions?: unknown[][]): void;
    /**
     * 重定向控制台输出到logger。
     *
     * @param logger - HvigorLogger。
     * @returns 原始控制台方法的快照。
     */
    static redirectConsole(logger: HvigorLogger): Console;
    /**
     * 恢复控制台方法到原始状态。
     *
     * @param consoleSnapshot 包含原始控制台方法的快照。
     */
    static restoreConsole(consoleSnapshot: Console): void;
}
export declare function evaluateLogLevel(level: Level, ignoreLevelCategoryFilterArr?: string[]): void;
/**
 * 这个方法会对configuration里加入daemon或者daemon-client的部分
 * 之后调用setConfiguration时，就会使用包含daemon/daemon-client的部分
 *
 * @param {Configuration} configuration
 */
export declare function configure(configuration: Configuration): void;
