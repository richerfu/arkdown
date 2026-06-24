/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/group-by-more-info.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as group_base_js_1 from "./group-base";
import * as merge_util_js_1 from "./merge-util";
export class GroupByMoreInfo extends group_base_js_1.GroupBase {
  constructor(r) {
    super(r, errorlog_consts_js_1.MergeKeyEnum.MORE_INFO);
  }
  _groupByKey(r, e, o) {
    merge_util_js_1.MergeUtil.composeErrors(errorlog_consts_js_1.MergeKeyEnum.MORE_INFO, r, e, o);
  }
}
