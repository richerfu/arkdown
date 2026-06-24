/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/perf-info.d.ts
 */
import { CompileEvent } from '../analyzer/compile-event';
import { AnalyzeMode } from '../util/constants';
/**
 * 性能分析相关的配置
 */
export interface PerfInfo {
    /**
     * 创建一个新的CompileEvent事件，开始事件
     *
     * @param eventName 事件名
     */
    startEvent?: (eventName: string) => CompileEvent;
    /**
     * 结束一个已有的CompileEvent事件
     *
     * @param eventId 事件id
     */
    endEvent?: (eventId: string) => void;
    /**
     * 是否需要性能打点
     */
    perf: AnalyzeMode;
}
