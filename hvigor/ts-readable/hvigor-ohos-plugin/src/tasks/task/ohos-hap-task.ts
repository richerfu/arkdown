/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/ohos-hap-task.js
 */
import * as ohos_module_task_js_1 from "./ohos-module-task";
export class OhosHapTask extends ohos_module_task_js_1.OhosModuleTask {
  constructor(s, e) {
    super(s, e), this.targetCompileMode = this.moduleModel.getCompileMode(s.getTargetData().getProduct());
  }
}
