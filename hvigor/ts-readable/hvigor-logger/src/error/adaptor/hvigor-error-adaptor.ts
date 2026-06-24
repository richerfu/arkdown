/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/hvigor-error-adaptor.js
 */
import * as abstract_adaptor_js_1 from "../abstract-adaptor";
import * as constant_js_1 from "../constant";
import * as hvigor_error_js_1 from "../errors/hvigor-error";
import * as type_js_1 from "../type";
import * as utils_js_1 from "../utils";
export class HvigorErrorAdaptor extends abstract_adaptor_js_1.ErrorAdaptor {
  constructor(r, o = "id") {
    super(), this._hvigorError = new hvigor_error_js_1.HvigorError(this.getErrorCodeJsonPath(), {
      field: o,
      value: r
    });
  }
  getErrorMessage() {
    var r;
    return [{
      timestamp: this._hvigorError.timestamp,
      id: this._hvigorError.id,
      code: this._hvigorError.code,
      originMessage: this._hvigorError.originMessage,
      originSolutions: this._hvigorError.originSolutions,
      moreInfo: this._hvigorError.moreInfo,
      stack: this._hvigorError.stack,
      message: null !== (r = this._hvigorError.message) && undefined !== r ? r : constant_js_1.ERROR_NOT_MATCH,
      solutions: this._hvigorError.solutions,
      checkMessage: this._hvigorError.checkMessage
    }];
  }
  getErrorCodeJsonPath() {
    return [(0, utils_js_1.getErrorInfoFilePath)(this.getToolErrorCode())];
  }
  getToolErrorCode() {
    return type_js_1.ToolErrorCode.TEC_10;
  }
  formatMessage(...r) {
    return this._hvigorError.formatMessage(...r), this;
  }
  formatSolutions(r, ...o) {
    return this._hvigorError.formatSolutions(r, ...o), this;
  }
  isMatchSuccess() {
    return this._hvigorError.isMatchSuccess();
  }
}
