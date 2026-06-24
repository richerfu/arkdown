/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/hvigor-global-error-adaptor.js
 */
import * as type_js_1 from "../type";
import * as utils_js_1 from "../utils";
import * as hvigor_error_adaptor_js_1 from "./hvigor-error-adaptor";
export class HvigorGlobalErrorAdaptor extends hvigor_error_adaptor_js_1.HvigorErrorAdaptor {
  constructor(r) {
    super(r, "checkMessage"), this._hvigorError.message ? "%s" === this._hvigorError.message && this.formatMessage(r) : this._hvigorError.message = r;
  }
  getErrorCodeJsonPath() {
    return [(0, utils_js_1.getErrorInfoFilePath)(type_js_1.ToolErrorCode.TEC_00)];
  }
}
