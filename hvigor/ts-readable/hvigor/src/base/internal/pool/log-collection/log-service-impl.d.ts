/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/log-collection/log-service-impl.d.ts
 */
import { Report } from '../../../common/report/report';
import { ReportListener } from '../../../common/report/report-listener';
import { LogType } from '../enum/log-type';
import { Log } from './log';
import { LogService } from './log-service';
/**
 * LogService实现类
 *
 * @since 2022/8/18
 */
export declare class LogServiceImpl implements LogService, ReportListener {
    private isActive;
    private logCollectorMap;
    constructor();
    clear(type?: LogType): void;
    getLog(type: LogType): Set<Log> | undefined;
    /**
     * 添加日志
     *
     * @param log 待添加的日志
     */
    addLog(log: Log): void;
    /**
     * 允许收集日志
     */
    start(): void;
    /**
     * 停止收集日志
     */
    end(): void;
    queryReport(): Report<Set<Log>>;
}
