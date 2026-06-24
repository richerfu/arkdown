/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/eight-digit-error-code-adaptor.js
 */
import * as abstract_adaptor_js_1 from "../abstract-adaptor";
import * as constant_js_1 from "../constant";
export class EightDigitErrorCodeAdaptor extends abstract_adaptor_js_1.ErrorAdaptor {
  constructor(r) {
    super(), this._errorMsg = r;
  }
  getErrorInfos() {
    var r;
    const t = new RegExp(constant_js_1.EIGHT_DIGIT_ERROR_REGEXP.PATTERN, constant_js_1.EIGHT_DIGIT_ERROR_REGEXP.FLAGS),
      o = this._errorMsg.matchAll(t),
      s = [];
    for (const t of o) {
      const o = {
        code: (null === (r = t.groups) || undefined === r ? undefined : r.code) || constant_js_1.DEFAULT_ERROR_CODE
      };
      s.push(o);
    }
    return s;
  }
  getErrorMessage() {
    const r = this.getErrorInfos().map(r => ({
      timestamp: new Date(),
      code: r.code,
      message: ""
    }));
    return r[0] && (r[0].message = this._errorMsg), r;
  }
}
