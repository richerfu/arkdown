/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/interfaces/create-group-by-key-factory.d.ts
 */
import { HvigorErrorInfo } from '../logger/hvigor-error-info';
import { GroupByKey } from './group-by-key';
export interface CreateGroupByKeyFactory {
    newGroupByKey(hvigorLogList: HvigorErrorInfo[]): GroupByKey;
}
