/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/metric-cache-service.d.ts
 */
import { MapCacheService } from './map-cache-service';
import { BaseEvent } from '../../../metrics/event/base-event';
/**
 * 提供获取TaskMetric缓存的统一服务接口
 *
 * @since 2022/9/1
 */
export declare class MetricCacheService extends MapCacheService<BaseEvent> {
    private static instance;
    private constructor();
    add(event: BaseEvent): void;
    getEvents(): BaseEvent[];
    static getInstance(): MetricCacheService;
}
