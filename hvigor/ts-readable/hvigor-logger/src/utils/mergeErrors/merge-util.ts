/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-util.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
export class MergeUtil {
  static init() {
    this.compareFuncMap.set(errorlog_consts_js_1.MergeKeyEnum.CODE, "compareCode"), this.compareFuncMap.set(errorlog_consts_js_1.MergeKeyEnum.CAUSE, "compareCause"), this.compareFuncMap.set(errorlog_consts_js_1.MergeKeyEnum.POSITION, "comparePosition"), this.compareFuncMap.set(errorlog_consts_js_1.MergeKeyEnum.SOLUTIONS, "compareSolutions"), this.compareFuncMap.set(errorlog_consts_js_1.MergeKeyEnum.MORE_INFO, "compareMoreInfo");
  }
  static composeErrors(e, r, o, s) {
    MergeUtil.compareFuncMap.size < 1 && MergeUtil.init(), r.forEach(r => {
      if (0 === s.size) return void s.set(Symbol(), [r]);
      let t = false;
      s.forEach((s, c) => {
        const n = this.compareFuncMap.get(e);
        n && o[n] && o[n](s[0], r) && (t = true, s.push(r));
      }), t || s.set(Symbol(), [r]);
    });
  }
}
MergeUtil.compareFuncMap = new Map();
