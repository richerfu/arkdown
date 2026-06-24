/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/report/report-listener.d.ts
 */
import { Report } from './report';
/**
 * 报告服务监听器接口
 *
 * @since 2022/8/18
 */
export interface ReportListener {
    queryReport(): Report<any>;
}
