/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/status.js
 */
import * as runtime_type_enum_js_1 from "../../enum/runtime-type-enum";
export class Status {
  constructor(t) {
    this._moduleBuildOpt = t, this._statusCode = this.initRuntimeStatus();
  }
  initRuntimeStatus() {
    return runtime_type_enum_js_1.RuntimeTypeEnum.valueOf(this._moduleBuildOpt.runtimeOS);
  }
  getStatusCode() {
    return this._statusCode;
  }
  is(t) {
    return (this._statusCode & t) === t;
  }
}
export class FullStatus {
  constructor(t) {
    this._fullStatusCode = 0, t.forEach(t => {
      this._fullStatusCode |= t.getStatusCode();
    });
  }
  has(t) {
    return (this._fullStatusCode & t) === t;
  }
}
