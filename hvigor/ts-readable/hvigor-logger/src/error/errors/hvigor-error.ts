/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/errors/hvigor-error.js
 */
import util_1 from "util";
import * as abstract_error_js_1 from "../abstract-error";
export class HvigorError extends abstract_error_js_1.BaseError {
  constructor(r, s) {
    super(r, s), this._errorInfo && (this._originMessage = this._errorInfo.message, this._message = this._errorInfo.message, this._originSolutions = this._errorInfo.solutions);
  }
  formatMessage(...r) {
    this.originMessage && (this._message = util_1.format(this._originMessage, ...r));
  }
  formatSolutions(r, ...s) {
    this._originSolutions && this._originSolutions[r] && this._solutions && (this._solutions[r] = util_1.format(this._originSolutions[r], ...s));
  }
  get originMessage() {
    return this._originMessage;
  }
  get originSolutions() {
    return this._originSolutions;
  }
}
