/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/abstract-module-hook-task.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../../const/common-const";
export class AbstractModuleHookTask extends hvigor_1.DefaultTask {
  constructor(o, s, e) {
    super(o.getModuleModel().getModule(), e), this.registryAction = () => () => {}, this._moduleService = o, this._isFaMode = s, this.getWorkerPool().warmUp(common_const_js_1.CommonConst.WARM_UP_SCRIPT);
  }
}
