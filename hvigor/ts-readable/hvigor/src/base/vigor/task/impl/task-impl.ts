/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/task/impl/task-impl.js
 */
import * as hvigor_trace_factory_js_1 from "../../../common/trace/hvigor-trace-factory";
var __decorate = this && this.__decorate || function (e, t, r, a) {
  var n,
    s = arguments.length,
    o = s < 3 ? t : null === a ? a = Object.getOwnPropertyDescriptor(t, r) : a;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) o = Reflect.decorate(e, t, r, a);else for (var _ = e.length - 1; _ >= 0; _--) (n = e[_]) && (o = (s < 3 ? n(o) : s > 3 ? n(t, r, o) : n(t, r)) || o);
  return s > 3 && o && Object.defineProperty(t, r, o), o;
};
export class TaskImpl {
  constructor(e) {
    this.task = e;
  }
  getDependencies() {
    return this._getDependencies();
  }
  getDependenciesInternal() {
    return this._getDependencies();
  }
  _getDependencies() {
    return this.task.getDependsOn().map(e => e && e.startsWith(":") ? e.substring(1, e.length) : e);
  }
  getName() {
    return this._getName();
  }
  getNameInternal() {
    return this._getName();
  }
  _getName() {
    return this.task.getName();
  }
  setEnable(e) {
    this._setEnable(e);
  }
  setEnableInternal(e) {
    this._setEnable(e);
  }
  _setEnable(e) {
    this.task.setEnabled(e);
  }
  beforeRun(e) {
    this._beforeRun(e);
  }
  beforeRunInternal(e) {
    this._beforeRun(e);
  }
  _beforeRun(e) {
    this.task.beforeRun(e);
  }
  afterRun(e) {
    this._afterRun(e);
  }
  afterRunInternal(e) {
    this._afterRun(e);
  }
  _afterRun(e) {
    this.task.afterRun(e);
  }
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], TaskImpl.prototype, "getDependencies", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], TaskImpl.prototype, "getName", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], TaskImpl.prototype, "setEnable", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], TaskImpl.prototype, "beforeRun", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], TaskImpl.prototype, "afterRun", null);
