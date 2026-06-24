/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/pack-tool-adaptor.js
 */
import * as type_js_1 from "../type";
import * as utils_js_1 from "../utils";
import * as third_party_error_adaptor_js_1 from "./third-party-error-adaptor";
export class PackToolAdaptor extends third_party_error_adaptor_js_1.ThirdPartyErrorAdaptor {
  constructor(r) {
    super(r);
  }
  analyze(r) {
    let t = "";
    r.split("\r\n").forEach(r => {
      if (r.includes(third_party_error_adaptor_js_1.ThirdPartyErrorAdaptor.SOLUTIONS)) {
        const t = r.split(third_party_error_adaptor_js_1.ThirdPartyErrorAdaptor.SOLUTIONS);
        t.length >= 2 && (this._thirdPartyError.solutions = t[1].split(">").map(r => r.trimEnd().trim()).filter(r => r.length > 0));
      } else t += `${r}\r\n`;
    }), this._thirdPartyError.message = t.trimEnd();
  }
  getErrorCodeJsonPath() {
    return [(0, utils_js_1.getErrorInfoFilePath)(type_js_1.ToolErrorCode.TEC_22)];
  }
}
