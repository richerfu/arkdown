/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/task/default-task.js
 */
import * as core_task_impl_js_1 from "./core-task-impl";
import * as wdk_1 from "@baize/wdk";
export class DefaultTask extends core_task_impl_js_1.CoreTaskImpl {
  constructor(e, t) {
    let s;
    s = "string" == typeof t ? {
      name: t || "default"
    } : t, super(e, s);
  }
  registryAction() {
    return wdk_1.noop;
  }
  getAction() {
    return this.registryAction();
  }
}
