/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/group-by-position.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as group_base_js_1 from "./group-base";
import * as merge_util_js_1 from "./merge-util";
export class GroupByPosition extends group_base_js_1.GroupBase {
  constructor(r) {
    super(r, errorlog_consts_js_1.MergeKeyEnum.POSITION);
  }
  _groupByKey(r, e, o) {
    merge_util_js_1.MergeUtil.composeErrors(errorlog_consts_js_1.MergeKeyEnum.POSITION, r, e, o);
  }
}
