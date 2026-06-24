/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sync/sync-module.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_2 from "@ohos/hvigor";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_module_model_bean_js_1 from "./ide/ohos-module-model-bean";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class SyncModule extends hvigor_1.DefaultSyncTask {
  constructor(e, o, s) {
    super(e, o), this.registryAction = () => () => {
      const e = new ohos_module_model_bean_js_1.OhosModuleModelBean(this.node, this.isFaMode);
      hvigor_2.defaultModelRegistry.registry(e);
    }, this.isFaMode = s, hvigor_1.hvigorCore.versionChanged() && this.dependsOn(CommonTask.CLEAN.name);
  }
}
