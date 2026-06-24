/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/ohos-app-task.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../../const/common-const";
export class OhosAppTask extends hvigor_1.IncrementalExecTask {
  constructor(s, o, e) {
    super(s, e), this.registryAction = () => this.doTaskAction, this.project = this.node, this.service = o, this.initTaskDepends(), this.getWorkerPool().warmUp(common_const_js_1.CommonConst.WARM_UP_SCRIPT);
  }
}
