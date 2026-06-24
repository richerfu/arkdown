/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/previewer/abstract-previewer-res.js
 */
import path_1 from "path";
import * as common_const_js_1 from "../../../const/common-const";
import * as task_names_js_1 from "../../common/task-names";
import * as abstract_module_hook_task_js_1 from "../abstract-module-hook-task";
var CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
export class AbstractPreviewerRes extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(e, o) {
    var s;
    super(e, o, CommonHookTask.BUILD_PREVIEWER_RES), this.syscapJsonPath = path_1.resolve(null === (s = e.getModuleModel()) || undefined === s ? undefined : s.getSourceRootByTargetName(), common_const_js_1.CommonConst.SYSCAP_JSON);
  }
}
