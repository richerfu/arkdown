/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/models/build-result.js
 */
import * as hvigor_trace_factory_js_1 from "../../common/trace/hvigor-trace-factory";
var __decorate = this && this.__decorate || function (r, e, t, o) {
  var s,
    c = arguments.length,
    n = c < 3 ? e : null === o ? o = Object.getOwnPropertyDescriptor(e, t) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(r, e, t, o);else for (var l = r.length - 1; l >= 0; l--) (s = r[l]) && (n = (c < 3 ? s(n) : c > 3 ? s(e, t, n) : s(e, t)) || n);
  return c > 3 && n && Object.defineProperty(e, t, n), n;
};
export class BuildResult {
  constructor(r, e = null) {
    this.error = r, this.report = e;
  }
  getError() {
    return this._getError();
  }
  getErrorInternal() {
    return this._getError();
  }
  _getError() {
    return this.error;
  }
  getReportJson() {
    return this._getReportJson();
  }
  getReportJsonInternal() {
    return this._getReportJson();
  }
  _getReportJson() {
    return this.report;
  }
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], BuildResult.prototype, "getError", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], BuildResult.prototype, "getReportJson", null);
