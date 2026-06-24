/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/log/adaptor/file-logger.d.ts
 */
import type { Level, Logger } from 'log4js';
/**
 * 替换打印内容中的bundleName
 * 若此函数涉及到大对象，需要使用者考虑下性能问题
 * @param msg 打印内容
 * @param bundleName bundleName
 * @param regExp 根据bundleName生成的正则
 */
export declare function replaceBundleName(msg: any, bundleName: string, regExp?: RegExp): any;
export declare class FileLogger {
    protected readonly fileLogger: Logger;
    constructor(logger: Logger);
    debug(message: any, ...args: any[]): any[];
    log(level: Level | string, ...args: unknown[]): void;
    warn(message: any, ...args: any[]): void;
    info(message: any, ...args: any[]): void;
    error(message: any, ...args: any[]): void;
}
