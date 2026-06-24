/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/group-by-code.d.ts
 */
import { MergeOption } from '../../interfaces/merge-option';
import { HvigorErrorInfo } from '../../logger/hvigor-error-info';
import { GroupBase } from './group-base';
/**
 * 按错误码分组
 */
export declare class GroupByCode extends GroupBase {
    constructor(hvigorLogList: HvigorErrorInfo[]);
    _groupByKey(hvigorLogList: HvigorErrorInfo[], option: MergeOption, res: Map<symbol, HvigorErrorInfo[]>): void;
}
