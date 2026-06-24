/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/log-collection/log.d.ts
 */
import { LogType } from '../enum/log-type';
/**
 * 日志
 */
export type Log = {
    type: LogType;
    time: string;
    workerId: number;
};
/**
 * 工作日志
 */
export interface WorkLog extends Log {
    level?: string;
    content: string;
    source?: string;
    taskPath?: string;
    taskName?: string;
    taskCompletePath?: string;
}
/**
 * 调度日志
 */
export interface ScheduleLog extends Log {
    workId: string;
}
/**
 * 生成调度日志
 *
 * @param time 时间
 * @param workId 工作单元id
 * @param workerId worker的id
 * @returns {ScheduleLog} 生成的调度日志
 */
export declare function generateScheduleLog(time: string, workId: string, workerId: number): ScheduleLog;
