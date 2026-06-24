/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/create-group-by-solutions.js
 */
import * as group_by_solutions_js_1 from "./group-by-solutions";
export class CreateGroupBySolutions {
  newGroupByKey(o) {
    return new group_by_solutions_js_1.GroupBySolutions(o);
  }
}
