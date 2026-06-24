/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/legacy/legacy-app-plugin.js
 */
import * as legacy_project_model_impl_js_1 from "../../model/project/legacy-project-model-impl";
import * as dependency_manager_js_1 from "../../project/dependency/dependency-manager";
import * as project_task_service_js_1 from "../../tasks/service/project-task-service";
import * as abstract_project_plugin_js_1 from "../common/abstract-project-plugin";
export class LegacyAppPlugin extends abstract_project_plugin_js_1.AbstractProjectPlugin {
  constructor(e) {
    super(e);
    const c = new legacy_project_model_impl_js_1.LegacyProjectModelImpl(e);
    c.initSubModules(), this._projectModel = c;
  }
  initTaskService() {
    const e = new dependency_manager_js_1.DependencyManager(true, this._projectModel);
    this._taskService = new project_task_service_js_1.ProjectTaskService(this._project, this._projectModel, e, true);
  }
}
