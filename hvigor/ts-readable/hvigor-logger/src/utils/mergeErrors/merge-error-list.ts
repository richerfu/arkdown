/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-error-list.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as hvigor_error_info_js_1 from "../../logger/hvigor-error-info";
import * as create_group_by_cause_js_1 from "./create-group-by-cause";
import * as create_group_by_code_js_1 from "./create-group-by-code";
import * as create_group_by_more_info_js_1 from "./create-group-by-more-info";
import * as create_group_by_position_js_1 from "./create-group-by-position";
import * as create_group_by_solutions_js_1 from "./create-group-by-solutions";
import * as merge_cause_js_1 from "./merge-cause";
import * as merge_code_js_1 from "./merge-code";
import * as merge_description_js_1 from "./merge-description";
import * as merge_more_info_js_1 from "./merge-more-info";
import * as merge_position_js_1 from "./merge-position";
import * as merge_solutions_js_1 from "./merge-solutions";
export class MergeErrorList {
  constructor(e, r, o) {
    this.hvigorLogList = e, this.mergeKey = r, this.option = this.handleOption(o), MergeErrorList.mergePropertyMap.set(errorlog_consts_js_1.MergeKeyEnum.CODE, new merge_code_js_1.MergeCode()), MergeErrorList.mergePropertyMap.set("description", new merge_description_js_1.MergeDescription()), MergeErrorList.mergePropertyMap.set(errorlog_consts_js_1.MergeKeyEnum.CAUSE, new merge_cause_js_1.MergeCause()), MergeErrorList.mergePropertyMap.set(errorlog_consts_js_1.MergeKeyEnum.POSITION, new merge_position_js_1.MergePosition()), MergeErrorList.mergePropertyMap.set(errorlog_consts_js_1.MergeKeyEnum.SOLUTIONS, new merge_solutions_js_1.MergeSolutions()), MergeErrorList.mergePropertyMap.set(errorlog_consts_js_1.MergeKeyEnum.MORE_INFO, new merge_more_info_js_1.MergeMoreInfo()), MergeErrorList.groupByKeyMap.set(errorlog_consts_js_1.MergeKeyEnum.CODE, new create_group_by_code_js_1.CreateGroupByCode()), MergeErrorList.groupByKeyMap.set(errorlog_consts_js_1.MergeKeyEnum.CAUSE, new create_group_by_cause_js_1.CreateGroupByCause()), MergeErrorList.groupByKeyMap.set(errorlog_consts_js_1.MergeKeyEnum.POSITION, new create_group_by_position_js_1.CreateGroupByPosition()), MergeErrorList.groupByKeyMap.set(errorlog_consts_js_1.MergeKeyEnum.SOLUTIONS, new create_group_by_solutions_js_1.CreateGroupBySolutions()), MergeErrorList.groupByKeyMap.set(errorlog_consts_js_1.MergeKeyEnum.MORE_INFO, new create_group_by_more_info_js_1.CreateGroupByMoreInfo());
  }
  getMergedErrorList() {
    const e = this.getGroupedErrors(this.mergeKey, this.option);
    let r = [];
    return e.forEach((e, o) => {
      r.push(this.mergeErrors(e, this.option));
    }), r;
  }
  getGroupedErrors(e, r) {
    var o;
    const s = new Map();
    return null === (o = MergeErrorList.groupByKeyMap.get(this.mergeKey)) || undefined === o || o.newGroupByKey(this.hvigorLogList).groupByKey(r, s), s;
  }
  mergeErrors(e, r) {
    let o = new hvigor_error_info_js_1.HvigorErrorInfo({
      code: e[0].getCode(),
      cause: e[0].getCause(),
      description: e[0].getDescription(),
      position: e[0].getPosition(),
      solutions: e[0].getSolutions(),
      moreInfo: e[0].getMoreInfo()
    });
    for (let s = 1; s < e.length; s++) {
      const t = e[s];
      o = this.mergeTwoErrors(o, t, r);
    }
    return o;
  }
  mergeTwoErrors(e, r, o) {
    const s = this.handleOption(o),
      t = new hvigor_error_info_js_1.HvigorErrorInfo();
    return MergeErrorList.mergePropertyMap.forEach((o, _) => {
      t[_] = o.mergeTwoKey(e, r, s);
    }), t;
  }
  handleOption(e) {
    const r = {
      code: errorlog_consts_js_1.MergeType.COLLECT_LAST,
      cause: errorlog_consts_js_1.MergeType.COLLECT_LAST,
      position: errorlog_consts_js_1.MergeType.COLLECT_LAST,
      solutions: errorlog_consts_js_1.MergeType.COLLECT_LAST,
      moreInfo: errorlog_consts_js_1.MergeType.COLLECT_LAST,
      compareCode: (e, r) => e.getCode() === r.getCode(),
      compareCause: (e, r) => e.getCause() === r.getCause(),
      comparePosition: (e, r) => e.getPosition() === r.getPosition(),
      compareMoreInfo: (e, r) => {
        var o, s, t, _;
        return !(!e || !r) && (null === (o = e.getMoreInfo()) || undefined === o ? undefined : o.cn) == (null === (s = r.getMoreInfo()) || undefined === s ? undefined : s.cn) && (null === (t = e.getMoreInfo()) || undefined === t ? undefined : t.en) == (null === (_ = r.getMoreInfo()) || undefined === _ ? undefined : _.en);
      },
      compareSolutions: (e, r) => {
        if (!e || !r) return false;
        const o = e.getSolutions().sort(),
          s = r.getSolutions().sort();
        return o.join() === s.join();
      }
    };
    return Object.assign(r, e);
  }
}
MergeErrorList.mergePropertyMap = new Map(), MergeErrorList.groupByKeyMap = new Map();
