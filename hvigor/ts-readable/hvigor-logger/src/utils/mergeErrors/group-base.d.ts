/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/group-base.d.ts
 */
import { MergeKeyEnum } from '../../consts/errorlog-consts';
import { GroupByKey } from '../../interfaces/group-by-key';
import { MergeOption } from '../../interfaces/merge-option';
import { HvigorErrorInfo } from '../../logger/hvigor-error-info';
/**
 * 分组的基类
 */
export declare abstract class GroupBase implements GroupByKey {
    private readonly log;
    private hvigorLogList;
    private mergeKey;
    constructor(hvigorLogList: HvigorErrorInfo[], mergeKey: MergeKeyEnum);
    groupByKey(option: MergeOption, res: Map<symbol, HvigorErrorInfo[]>): void;
    abstract _groupByKey(hvigorLogList: HvigorErrorInfo[], option: MergeOption, res: Map<symbol, HvigorErrorInfo[]>): void;
}
