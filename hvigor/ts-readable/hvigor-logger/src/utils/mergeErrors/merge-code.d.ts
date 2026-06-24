/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-code.d.ts
 */
import { MergeOption } from '../../interfaces/merge-option';
import { MergeTwoProperty } from '../../interfaces/merge-two-property';
import { HvigorErrorInfo } from '../../logger/hvigor-error-info';
import { MoreInfo } from '../../types/error-log-types';
export declare class MergeCode implements MergeTwoProperty {
    private readonly log;
    mergeTwoKey(firstError: HvigorErrorInfo, secondError: HvigorErrorInfo, option: MergeOption): string | string[] | MoreInfo;
}
