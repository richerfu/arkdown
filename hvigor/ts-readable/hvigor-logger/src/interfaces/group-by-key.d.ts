/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/interfaces/group-by-key.d.ts
 */
import { MergeOption } from './merge-option';
import { HvigorErrorInfo } from '../logger/hvigor-error-info';
/**
 * 分组的接口
 */
export interface GroupByKey {
    /**
     * 按某个属性进行合并时，实现对数据的分组，分组数据通过入参res带出
     * @param option
     * @param res
     */
    groupByKey(option: MergeOption, res: Map<symbol, HvigorErrorInfo[]>): void;
}
