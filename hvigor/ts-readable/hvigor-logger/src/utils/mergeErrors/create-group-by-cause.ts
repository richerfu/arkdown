/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/create-group-by-cause.js
 */
import * as group_by_cause_js_1 from "./group-by-cause";
export class CreateGroupByCause {
  newGroupByKey(e) {
    return new group_by_cause_js_1.GroupByCause(e);
  }
}
