/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/third-party-error-adaptor.js
 */
import * as abstract_adaptor_js_1 from "../abstract-adaptor";
import * as constant_js_1 from "../constant";
import * as third_party_error_js_1 from "../errors/third-party-error";
import * as type_js_1 from "../type";
import * as utils_js_1 from "../utils";
export class ThirdPartyErrorAdaptor extends abstract_adaptor_js_1.ErrorAdaptor {
  constructor(r, t = "checkMessage") {
    if (super(), this._thirdPartyError = new third_party_error_js_1.ThirdPartyError(this.getErrorCodeJsonPath(), {
      field: t,
      value: r
    }), "checkMessage" === t) this.errorMessage = r, this.analyze(r);else if ("none" === t) {
      const t = r;
      this._thirdPartyError.code = t.code, this._thirdPartyError.message = t.message, this._thirdPartyError.solutions = t.solutions, this._thirdPartyError.toolName = t.toolName;
    }
  }
  analyze(r) {
    if (!r.includes(ThirdPartyErrorAdaptor.SOLUTIONS)) return;
    const t = r.split(ThirdPartyErrorAdaptor.SOLUTIONS);
    this._thirdPartyError.message = t[0].trimEnd(), this._thirdPartyError.solutions = t[1].split(">").map(r => r.trimEnd().trim()).filter(r => r.length > 0), this.checkEs2abc();
  }
  checkEs2abc() {
    const r = this._thirdPartyError.solutions;
    if (r && r[(null == r ? undefined : r.length) - 1].includes("The size of programs is expected to be")) {
      const t = r[(null == r ? undefined : r.length) - 1],
        s = t.indexOf("The size of programs is expected");
      r[(null == r ? undefined : r.length) - 1] = `${t.substring(0, s)}\n\n${t.substring(s)}`, this._thirdPartyError.solutions = r;
    }
  }
  getErrorMessage() {
    var r, t;
    return [{
      timestamp: this._thirdPartyError.timestamp,
      id: this._thirdPartyError.id,
      code: this._thirdPartyError.code,
      moreInfo: this._thirdPartyError.moreInfo,
      stack: this._thirdPartyError.stack,
      message: null !== (t = null !== (r = this._thirdPartyError.message) && undefined !== r ? r : this.errorMessage) && undefined !== t ? t : constant_js_1.ERROR_NOT_MATCH,
      solutions: this._thirdPartyError.solutions,
      checkMessage: this._thirdPartyError.checkMessage
    }];
  }
  getErrorCodeJsonPath(r) {
    return r ? [(0, utils_js_1.getErrorInfoFilePath)(r)] : ThirdPartyErrorAdaptor.otherErrorCode.map(r => (0, utils_js_1.getErrorInfoFilePath)(r));
  }
}
ThirdPartyErrorAdaptor.SOLUTIONS = "Solutions:", ThirdPartyErrorAdaptor.otherErrorCode = [type_js_1.ToolErrorCode.TEC_23, type_js_1.ToolErrorCode.TEC_24];
