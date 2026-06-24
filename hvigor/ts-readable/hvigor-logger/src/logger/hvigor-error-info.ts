/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-error-info.js
 */
import * as errorlog_consts_js_1 from "../consts/errorlog-consts";
export class HvigorErrorInfo {
  constructor(o) {
    var s, r, t, e, i;
    this.code = errorlog_consts_js_1.UNDEFINED_CODE, this.description = errorlog_consts_js_1.UNDEFINED_DESC, this.cause = errorlog_consts_js_1.UNDEFINED_CAUSE, this.position = errorlog_consts_js_1.UNDEFINED_POS, this.solutions = [], o && (this.code = null !== (s = o.code) && undefined !== s ? s : errorlog_consts_js_1.UNDEFINED_CODE, this.description = null !== (r = o.description) && undefined !== r ? r : errorlog_consts_js_1.UNDEFINED_DESC, this.cause = null !== (t = o.cause) && undefined !== t ? t : errorlog_consts_js_1.UNDEFINED_CAUSE, this.position = null !== (e = o.position) && undefined !== e ? e : errorlog_consts_js_1.UNDEFINED_POS, this.solutions = null !== (i = o.solutions) && undefined !== i ? i : [], this.moreInfo = o.moreInfo);
  }
  getCode() {
    return this.code;
  }
  getDescription() {
    return this.description;
  }
  getCause() {
    return this.cause;
  }
  getPosition() {
    return this.position;
  }
  getSolutions() {
    return this.solutions;
  }
  getMoreInfo() {
    return this.moreInfo;
  }
  checkInfo() {
    return this.checkCode() && this.checkCause();
  }
  checkCode() {
    if (!this.code) return false;
    if (8 !== this.code.length) return false;
    return !!/[0-9]{8}/g.test(this.code);
  }
  checkCause() {
    return !!this.cause && (this.code === errorlog_consts_js_1.UNDEFINED_CODE ? this.cause === errorlog_consts_js_1.UNDEFINED_CAUSE : "" !== this.cause && undefined !== this.cause && null !== this.cause);
  }
}
