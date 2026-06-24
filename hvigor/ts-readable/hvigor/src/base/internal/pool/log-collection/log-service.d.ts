/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/log-collection/log-service.d.ts
 */
import { LogType } from '../enum/log-type';
import { Log } from './log';
/**
 * 日志服务
 *
 * @since 2022/8/18
 */
export interface LogService {
    /**
     * 获得日志
     *
     * @param type 日志类型
     * @returns {Set<Log>|undefined} 日志集合
     */
    getLog(type: LogType): Set<Log> | undefined;
    /**
     * 清空日志
     *
     * @param type 日志类型
     */
    clear(type?: LogType): void;
}
