/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-process-resource-increment.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as abstract_process_resource_js_1 from "./abstract-process-resource";
var __decorate = this && this.__decorate || function (e, r, t, c) {
  var s,
    o = arguments.length,
    n = o < 3 ? r : null === c ? c = Object.getOwnPropertyDescriptor(r, t) : c;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(e, r, t, c);else for (var u = e.length - 1; u >= 0; u--) (s = e[u]) && (n = (o < 3 ? s(n) : o > 3 ? s(r, t, n) : s(r, t)) || n);
  return o > 3 && n && Object.defineProperty(r, t, n), n;
};
export class AbstractProcessResourceIncrement extends abstract_process_resource_js_1.AbstractProcessResource {
  constructor(e, r) {
    super(e, r);
  }
  doTaskAction() {}
  declareInputs() {
    return new Map();
  }
  declareInputFiles() {
    return new hvigor_1.FileSet();
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet();
  }
}
__decorate([(0, hvigor_1.IgnoreSuperIncremental)()], AbstractProcessResourceIncrement.prototype, "declareInputs", null), __decorate([(0, hvigor_1.IgnoreSuperIncremental)()], AbstractProcessResourceIncrement.prototype, "declareInputFiles", null), __decorate([(0, hvigor_1.IgnoreSuperIncremental)()], AbstractProcessResourceIncrement.prototype, "declareOutputFiles", null);
