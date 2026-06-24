/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/assemble/assemble-app.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../../../const/common-const";
import * as task_names_js_1 from "../../common/task-names";
import * as sign_app_js_1 from "../../sign-app";
var CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
export class AssembleApp extends hvigor_1.DefaultTask {
  constructor(s, e) {
    super(s, CommonHookTask.ASSEMBLE_APP), this.registryAction = () => () => {}, this._taskService = e, this.initTaskDepends(), this.getWorkerPool().warmUp(common_const_js_1.CommonConst.WARM_UP_SCRIPT);
  }
  initTaskDepends() {
    this.dependsOn(sign_app_js_1.SignApp.name);
  }
}
