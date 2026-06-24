/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/group-by-cause.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as group_base_js_1 from "./group-base";
import * as merge_util_js_1 from "./merge-util";
export class GroupByCause extends group_base_js_1.GroupBase {
  constructor(e) {
    super(e, errorlog_consts_js_1.MergeKeyEnum.CAUSE);
  }
  _groupByKey(e, r, s) {
    merge_util_js_1.MergeUtil.composeErrors(errorlog_consts_js_1.MergeKeyEnum.CAUSE, e, r, s);
  }
}
