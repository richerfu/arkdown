/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/metric-service.d.ts
 */
import { Report } from '../common/report/report';
import { ReportListener } from '../common/report/report-listener';
import { BaseEvent } from './event/base-event';
/**
 * 矩阵服务
 *
 * @since 2022/8/17
 */
export declare class MetricService implements ReportListener {
    private static instance;
    private metricCacheService;
    private constructor();
    /**
     * 提交metric到缓存服务
     *
     * @param event
     */
    submit(event: BaseEvent): void;
    getEventById(id?: string): BaseEvent | undefined;
    /**
     * 供报告服务查询报告
     */
    queryReport(): Report<BaseEvent[]>;
    filterDurationEvent(events: BaseEvent[]): BaseEvent[];
    filterLogEvent(events: BaseEvent[]): BaseEvent[];
    /**
     * 清除缓存
     */
    clear(): void;
    static getInstance(): MetricService;
}
