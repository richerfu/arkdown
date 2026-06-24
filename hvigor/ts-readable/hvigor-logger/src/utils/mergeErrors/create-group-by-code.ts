/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/create-group-by-code.js
 */
import * as group_by_code_js_1 from "./group-by-code";
export class CreateGroupByCode {
  newGroupByKey(e) {
    return new group_by_code_js_1.GroupByCode(e);
  }
}
