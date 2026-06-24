/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/common/abstract-project-plugin.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as build_option_path_info_js_1 from "../../common/build-option-path-info";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as ohos_trace_js_1 from "../../common/trace/ohos-trace";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as clean_js_1 from "../../tasks/common/clean";
import * as task_names_js_1 from "../../tasks/common/task-names";
import * as assemble_app_js_1 from "../../tasks/hook/assemble/assemble-app";
import * as project_inspection_js_1 from "../../tasks/inspection/project-inspection";
import * as sync_project_js_1 from "../../tasks/sync/sync-project";
import * as ohos_project_context_impl_js_1 from "../context/impl/ohos-project-context-impl";
import * as ohos_plugin_id_js_1 from "./ohos-plugin-id";
var SyncTask = task_names_js_1.TaskNames.SyncTask;
export class AbstractProjectPlugin extends hvigor_1.HvigorSystemPlugin {
  constructor(e) {
    super(ohos_plugin_id_js_1.OhosPluginId.OHOS_APP_PLUGIN), this._project = e, this._projectName = e.getName();
  }
  doRegisterTask() {
    this.assembleApp = new assemble_app_js_1.AssembleApp(this._project, this._taskService), this.clean = new clean_js_1.Clean(this._project, this._taskService), this.syncProject = new sync_project_js_1.SyncProject(this._project, SyncTask.SYNC_TASK), this._project.registry(this.assembleApp), this._project.registry(this.clean), this._project.registry(this.syncProject);
  }
  doProjectInspection() {
    new project_inspection_js_1.ProjectInspection(this._projectModel).doInspection().exitOnError();
  }
  initBuildOptionMap(e) {
    build_mode_manager_js_1.buildOptionManager.initProjectBuildOptions(this._projectModel, e), build_option_path_info_js_1.buildOptionPath.initSignConfig(this._projectModel, e);
  }
  initTraceData() {
    ohos_trace_js_1.ohosTrace.initOhosTrace();
  }
  initGlobalData() {
    var e;
    const t = null === (e = this._project.getConfigOpt()) || undefined === e ? undefined : e.getObject("ohos");
    t && project_ohos_config_manager_js_1.projectOhosConfigManager.loaderConfig(t);
  }
  getTaskService() {
    return this._taskService;
  }
  getProjectModel() {
    return this._projectModel;
  }
  initContext() {
    this.ohosAppContext = new ohos_project_context_impl_js_1.OhosProjectContextImpl(this, this._taskService);
  }
  getContext() {
    return this.ohosAppContext;
  }
}
