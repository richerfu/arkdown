/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-util.d.ts
 */
import { MergeKeyEnum } from '../../consts/errorlog-consts';
import { MergeOption } from '../../interfaces/merge-option';
import { HvigorErrorInfo } from '../../logger/hvigor-error-info';
export declare class MergeUtil {
    private static readonly compareFuncMap;
    private static init;
    /**
     * 按mergeKey比较相等的两个错误对象，合并到一个数组res中。
     * @param mergeKey 错误信息合并对象的主键
     * @param hvigorLogList 全量错误信息列表
     * @param option 合并规则
     * @param res 结果收集
     */
    static composeErrors(mergeKey: MergeKeyEnum, hvigorLogList: HvigorErrorInfo[], option: MergeOption, res: Map<symbol, HvigorErrorInfo[]>): void;
}
