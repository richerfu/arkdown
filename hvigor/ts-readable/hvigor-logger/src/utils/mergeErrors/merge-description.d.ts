/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-description.d.ts
 */
import { MergeOption } from '../../interfaces/merge-option';
import { MergeTwoProperty } from '../../interfaces/merge-two-property';
import { HvigorErrorInfo } from '../../logger/hvigor-error-info';
import { MoreInfo } from '../../types/error-log-types';
/**
 * 描述不可被用户单独控制，描述跟随错误码的规则进行合并。
 */
export declare class MergeDescription implements MergeTwoProperty {
    private readonly log;
    mergeTwoKey(firstError: HvigorErrorInfo, secondError: HvigorErrorInfo, option: MergeOption): string | string[] | MoreInfo;
}
