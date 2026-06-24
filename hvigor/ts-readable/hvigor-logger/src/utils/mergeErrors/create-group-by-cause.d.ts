/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/create-group-by-cause.d.ts
 */
import { CreateGroupByKeyFactory } from '../../interfaces/create-group-by-key-factory';
import { GroupByKey } from '../../interfaces/group-by-key';
import { HvigorErrorInfo } from '../../logger/hvigor-error-info';
export declare class CreateGroupByCause implements CreateGroupByKeyFactory {
    newGroupByKey(hvigorLogList: HvigorErrorInfo[]): GroupByKey;
}
