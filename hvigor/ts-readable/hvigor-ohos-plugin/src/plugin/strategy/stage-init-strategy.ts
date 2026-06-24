/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/strategy/stage-init-strategy.js
 */
import * as initialization_strategy_js_1 from "./initialization-strategy";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as common_const_js_1 from "../../const/common-const";
import * as ohos_plugin_id_js_1 from "../common/ohos-plugin-id";
import * as app_plugin_js_1 from "../app-plugin";
import * as cangjie_register_js_1 from "../hooks/cangjie-register";
import * as hap_plugin_js_1 from "../hap-plugin";
import * as hsp_plugin_js_1 from "../hsp-plugin";
import * as har_plugin_js_1 from "../har-plugin";
import * as task_initializer_js_1 from "../../utils/task-initializer";
import * as ohos_logger_1 from "../../utils/log/ohos-logger";
export class StageInitializationStrategy extends initialization_strategy_js_1.InitializationStrategy {
  constructor() {
    super(), this._log = ohos_logger_1.OhosLogger.getLogger(StageInitializationStrategy.name);
  }
  createAppPlugin(i) {
    const e = new app_plugin_js_1.AppPlugin(i);
    return e.doProjectInspection(), e;
  }
  createHapPlugin(i, e) {
    var t;
    const a = new cangjie_register_js_1.CangjieRegisterService();
    this.cangjieObj = a.dealCangjieSchema();
    const n = this.initPluginParam(new hap_plugin_js_1.HapPlugin(i), i, true);
    n.initGlobalData(), this.setUpModulePlugin(n, i), e.creatorManager = task_initializer_js_1.TaskInitializer.initializeHap(n);
    try {
      null === (t = this.cangjieObj) || undefined === t || t.registerCangjieHapTask(i, n);
    } catch (i) {
      this._log.debug(`failed to register the cangjie hap task: ${i}`);
    }
    return n;
  }
  createHspPlugin(i, e) {
    var t;
    const a = new cangjie_register_js_1.CangjieRegisterService();
    this.cangjieObj = a.dealCangjieSchema();
    const n = this.initPluginParam(new hsp_plugin_js_1.HspPlugin(i), i, true);
    this.setUpModulePlugin(n, i), e.creatorManager = task_initializer_js_1.TaskInitializer.initializeHsp(n), e.initHspPluginGlobalData(i);
    try {
      null === (t = this.cangjieObj) || undefined === t || t.registerCangjieHspTask(i, n);
    } catch (i) {
      this._log.debug(`failed to register the cangjie hsp task: ${i}`);
    }
    return n;
  }
  createHarPlugin(i, e) {
    var t;
    const a = new cangjie_register_js_1.CangjieRegisterService();
    this.cangjieObj = a.dealCangjieSchema();
    const n = this.initPluginParam(new har_plugin_js_1.HarPlugin(i), i, false);
    n.initGlobalData(), this.setUpModulePlugin(n, i), e.creatorManager = task_initializer_js_1.TaskInitializer.initializeHar(n);
    try {
      null === (t = this.cangjieObj) || undefined === t || t.registerCangjieHarTask(i, n);
    } catch (i) {
      this._log.debug(`failed to register the cangjie har task: ${i}`);
    }
    return n;
  }
  createProjectModel(i) {
    const e = i.getProject(),
      t = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_APP_PLUGIN);
    return t || this._log.printErrorExit("STAGE_NO_SYSTEM_PLUGIN", [`${[e.getBuildFilePath()]}.ts`], [[`${[e.getBuildFilePath()]}.ts`]]), t.getProjectModel();
  }
  initPluginParam(i, e, t) {
    const a = this.createProjectModel(e),
      n = this.createModuleModel(a, e, t);
    return i.withProjectModel(a).withModuleModel(n).withPluginApiType(hap_extra_info_js_1.ApiType.STAGE).withRuntimeConfigFileName(common_const_js_1.CommonConst.MODULE_JSON5);
  }
  setUpModulePlugin(i, e) {
    i.doModuleInspection(), i.initModuleTaskService(false), i.initBuildOptionMap(e.getBuildProfilePath()), i.initTargetDepends(), i.initModuleCommonTasksForDiffApiType(false);
  }
}
