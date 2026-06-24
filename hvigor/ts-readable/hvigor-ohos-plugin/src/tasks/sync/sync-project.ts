/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sync/sync-project.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_project_model_bean_js_1 from "./ide/ohos-project-model-bean";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class SyncProject extends hvigor_1.DefaultSyncTask {
  constructor(e, o) {
    super(e, o), this.registryAction = () => () => {
      const e = new ohos_project_model_bean_js_1.OhosProjectModelBean(this.node);
      hvigor_1.defaultModelRegistry.registry(e);
    }, hvigor_1.hvigorCore.versionChanged() && this.dependsOn(CommonTask.CLEAN.name);
  }
}
