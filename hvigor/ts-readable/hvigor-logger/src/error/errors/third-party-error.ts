/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/errors/third-party-error.js
 */
import * as abstract_error_js_1 from "../abstract-error";
export class ThirdPartyError extends abstract_error_js_1.BaseError {
  constructor(r, e) {
    super(r, e), this._errorInfo && (this._toolName = this._errorInfo.toolName), "checkMessage" === e.field && (this._message = e.value);
  }
  get toolName() {
    return this._toolName;
  }
  set toolName(r) {
    this._toolName = r;
  }
}
