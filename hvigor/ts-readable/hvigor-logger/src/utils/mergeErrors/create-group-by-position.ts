/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/create-group-by-position.js
 */
import * as group_by_position_js_1 from "./group-by-position";
export class CreateGroupByPosition {
  newGroupByKey(o) {
    return new group_by_position_js_1.GroupByPosition(o);
  }
}
