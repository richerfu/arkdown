/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/task/interface/input-output.js
 */
import * as hvigor_trace_factory_js_1 from "../../../common/trace/hvigor-trace-factory";
var __decorate = this && this.__decorate || function (t, e, r, s) {
  var i,
    a = arguments.length,
    l = a < 3 ? e : null === s ? s = Object.getOwnPropertyDescriptor(e, r) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) l = Reflect.decorate(t, e, r, s);else for (var o = t.length - 1; o >= 0; o--) (i = t[o]) && (l = (a < 3 ? i(l) : a > 3 ? i(e, r, l) : i(e, r)) || l);
  return a > 3 && l && Object.defineProperty(e, r, l), l;
};
export class TaskInput {
  constructor(t, e) {
    this.declareInputs = t, this.declareInputFiles = e;
  }
  file(t) {
    return this._file(t);
  }
  fileInternal(t) {
    return this._file(t);
  }
  _file(t) {
    return this.declareInputFiles.addEntry(t), this;
  }
  files(t) {
    return this._files(t);
  }
  filesInternal(t) {
    return this._files(t);
  }
  _files(t) {
    return this.declareInputFiles.addEntries(t), this;
  }
  property(t, e) {
    return this._property(t, e);
  }
  propertyInternal(t, e) {
    return this._property(t, e);
  }
  _property(t, e) {
    return this.declareInputs.set(t, e), this;
  }
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], TaskInput.prototype, "file", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], TaskInput.prototype, "files", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], TaskInput.prototype, "property", null);
export class TaskOutput {
  constructor(t) {
    this.declareOutputFiles = t;
  }
  file(t) {
    return this._file(t);
  }
  fileInternal(t) {
    return this._file(t);
  }
  _file(t) {
    return this.declareOutputFiles.addEntry(t), this;
  }
  files(t) {
    return this._files(t);
  }
  filesInternal(t) {
    return this._files(t);
  }
  _files(t) {
    return this.declareOutputFiles.addEntries(t), this;
  }
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], TaskOutput.prototype, "file", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], TaskOutput.prototype, "files", null);
