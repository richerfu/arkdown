/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/report/report-service.d.ts
 */
import { ReportListener } from './report-listener';
/**
 * 报告服务接口
 *
 * @since 2022/8/18
 */
export interface ReportService {
    report(): void;
    getReport(): any;
    addListener(listener: ReportListener): void;
    removeListener(listener: ReportListener): void;
    startProcessMonitor(): void;
    stopProcessMonitor(): void;
}
