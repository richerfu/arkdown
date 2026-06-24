/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/common/abstract-module-plugin.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as build_option_util_js_1 from "../../project/build-option/build-option-util";
import * as clean_js_1 from "../../tasks/common/clean";
import * as task_names_js_1 from "../../tasks/common/task-names";
import * as compile_native_hook_js_1 from "../../tasks/hook/native/compile-native-hook";
import * as module_inspection_js_1 from "../../tasks/inspection/module-inspection";
import * as module_task_service_factory_js_1 from "../../tasks/service/module-task-service-factory";
import * as target_task_service_js_1 from "../../tasks/service/target-task-service";
import * as sync_module_js_1 from "../../tasks/sync/sync-module";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_plugin_id_js_1 from "./ohos-plugin-id";
import * as ohos_module_context_impl_js_1 from "../context/impl/ohos-module-context-impl";
var SyncTask = task_names_js_1.TaskNames.SyncTask;
var CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
const ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET;
export class AbstractModulePlugin extends hvigor_1.HvigorSystemPlugin {
  constructor(e, t) {
    super(e), this._log = ohos_logger_js_1.OhosLogger.getLogger(AbstractModulePlugin.name), this._module = t, this.moduleName = this._module.getName(), this._needExecTargetServiceList = [];
  }
  refreshTargetServiceList() {
    this._needExecTargetServiceList.forEach(e => {
      e.refreshBuildOption();
    });
  }
  getContext() {
    return this.ohosModuleContext;
  }
  initContext() {
    this.ohosModuleContext = new ohos_module_context_impl_js_1.OhosModuleContextImpl(this);
  }
  withProjectModel(e) {
    return this._projectModel = e, this;
  }
  getProjectModel() {
    return this._projectModel;
  }
  withModuleModel(e) {
    return this._moduleModel = e, this;
  }
  getModuleModel() {
    return this._moduleModel;
  }
  getModuleName() {
    return this.moduleName;
  }
  withPluginApiType(e) {
    return this._pluginApiType = e, this;
  }
  withRuntimeConfigFileName(e) {
    return this._configFileName = e, this;
  }
  initModuleTaskService(e) {
    this.checkModuleConfigStatus(), this._moduleService = module_task_service_factory_js_1.ModuleTaskServiceFactory.initModuleTaskService(this._projectModel, this._moduleModel, e);
  }
  initBuildOptionMap(e) {
    build_option_util_js_1.BuildOptionUtil.validateBuildOptionSet(this._moduleModel), build_option_util_js_1.BuildOptionUtil.validateBinderSet(this._moduleModel), build_mode_manager_js_1.buildOptionManager.initModuleBuildOptions(this._moduleModel, e);
  }
  doModuleInspection() {
    new module_inspection_js_1.ModuleInspection(this._moduleModel).doInspection().exitOnError();
  }
  getNeedExecTargetServiceList() {
    return this._needExecTargetServiceList;
  }
  initTargetDepends() {
    var e;
    let t,
      o = false;
    for (const i of this._moduleService.getTargetDataSet()) {
      const s = i[0];
      if (t = s.getProduct().name, i[1]) {
        o = true;
        const t = new target_task_service_js_1.TargetTaskService(s, this._moduleService);
        this._needExecTargetServiceList.push(t), null === (e = this._projectModel) || undefined === e || e.registryTarget(t);
      }
    }
    this._moduleService.getModuleModel().isHarModule() || o || this._log.warn(`Current product is '${t}'. No output will be generated because of no executable target in module: '${this._module.getName()}.'`);
  }
  initModuleCommonTasks() {
    this.clean = new clean_js_1.Clean(this._module, this._moduleService), this._module.registry(this.clean), this.compileNative = new compile_native_hook_js_1.CompileNativeHook(this._moduleService), this._module.registry(this.compileNative);
  }
  initModuleCommonTasksForDiffApiType(e) {
    this.sync = new sync_module_js_1.SyncModule(this._module, SyncTask.SYNC_TASK, e), this._module.registry(this.sync);
  }
  initTaskDependsForAllNeedExecTarget(...e) {
    e.forEach(e => {
      this._needExecTargetServiceList.forEach(t => {
        e.initTaskDepends(t);
      });
    });
  }
  checkModuleTarget() {
    this.checkHvigorCommandTarget(), this.checkTargetSourceCodeConfigFile();
  }
  checkHvigorCommandTarget() {
    const e = this._projectModel.getModuleSpecificTargets().get(this._moduleModel.getName());
    e && e.filter(e => "all" !== e).forEach(e => {
      this._moduleModel.getTargetOptions().filter(t => t.name === e).length > 0 || this._log.printErrorExit("TARGET_NOT_EXIST_IN_COMMAND", [e, this._module.getName()]), e === ohosTestTargetName && this.checkOhosTestTarget();
    });
  }
  checkTargetSourceCodeConfigFile() {
    this._moduleModel.getTargetOptions().forEach(e => {
      const t = e.name,
        o = this._moduleModel.getSourceRootByTargetName(t),
        i = path_1.resolve(o, this._configFileName);
      if (t === ohosTestTargetName) return;
      if (fs_extra_1.existsSync(i)) return;
      const s = `The configuration file '${i}' for the target '${t}' is not found in the '${this.moduleName}' module.`,
        r = `Ensure that the '${path_1.relative(this.moduleName, o)}' directory has been created for the module and the '${this._configFileName}' file exists in the directory.`;
      this._log._buildError(s)._solution(r)._printErrorAndExit();
    });
  }
  checkModuleConfigStatus() {
    const e = this.pluginId === ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN,
      t = this._pluginApiType;
    undefined !== this._moduleModel && this._moduleModel.getApiType() === t || this._log.printErrorExit("MISMATCH_API_TYPE", [this._moduleModel ? this._moduleModel.getName() : "project"], [[t, this._module.getBuildProfilePath()]]), this.checkModuleTarget(), this._moduleModel.isHapModule() !== e && this._log.printErrorExit("PLUGIN_DOES_NOT_MATCH_MODULE", [], [[this._moduleModel.getName(), `${this._module.getBuildFilePath()}.ts`]]);
  }
  checkOhosTestTarget() {
    const e = this._moduleModel.isHarModule(),
      t = this._moduleModel.isHspModule(),
      o = e || t ? e ? CommonHookTask.ASSEMBLE_HAR.name : CommonHookTask.ASSEMBLE_HSP.name : "";
    o && hvigor_1.hvigorCore.isCommandEntryTask(o) && this._log.printErrorExit("OHOSTEST_NOT_SUPPORT_HAR_HSP", [o, ohosTestTargetName, e ? "HAR" : "HSP"], [[], [], [hvigor_1.hvigorCore.isCommandEntryTask(CommonHookTask.GEN_ON_DEVICE_TEST_HAP.name) ? `Remove it and run '${CommonHookTask.GEN_ON_DEVICE_TEST_HAP.name}' alone.` : `Run '${CommonHookTask.GEN_ON_DEVICE_TEST_HAP.name}' instead.`]]);
    const i = this._moduleModel.getSourceRootByTargetName(ohosTestTargetName);
    fs_extra_1.existsSync(path_1.resolve(i, this._configFileName)) || this._log.printErrorExit("OHOSTEST_CONFIG_PROFILE_NOT_FOUND", [this.moduleName, this.moduleName, this._configFileName], [[this.moduleName, this.moduleName, this._configFileName, this._moduleModel.getProfilePath()]]);
  }
  getTaskService() {
    return this._moduleService;
  }
}
