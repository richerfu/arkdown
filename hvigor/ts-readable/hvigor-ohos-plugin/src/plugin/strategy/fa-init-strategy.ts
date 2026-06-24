/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/strategy/fa-init-strategy.js
 */
import * as initialization_strategy_js_1 from "./initialization-strategy";
import * as legacy_hap_plugin_js_1 from "../legacy/legacy-hap-plugin";
import * as legacy_har_plugin_js_1 from "../legacy/legacy-har-plugin";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as common_const_js_1 from "../../const/common-const";
import * as ohos_plugin_id_js_1 from "../common/ohos-plugin-id";
import * as legacy_app_plugin_js_1 from "../legacy/legacy-app-plugin";
import * as legacy_task_initializer_js_1 from "../../tasks/legacy-tasks/create/legacy-task-initializer";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class FaInitializationStrategy extends initialization_strategy_js_1.InitializationStrategy {
  constructor() {
    super(), this._log = ohos_logger_js_1.OhosLogger.getLogger(FaInitializationStrategy.name);
  }
  createAppPlugin(i) {
    const e = new legacy_app_plugin_js_1.LegacyAppPlugin(i);
    return e.doProjectInspection(), e;
  }
  createHapPlugin(i) {
    const e = this.initPluginParam(new legacy_hap_plugin_js_1.LegacyHapPlugin(i), i, true);
    return e.initGlobalData(), this.setUpModulePlugin(e, i), (0, legacy_task_initializer_js_1.legacyHapTasksInitialize)(e), e;
  }
  createHarPlugin(i) {
    const e = this.initPluginParam(new legacy_har_plugin_js_1.LegacyHarPlugin(i), i, false);
    return e.initGlobalData(), this.setUpModulePlugin(e, i), (0, legacy_task_initializer_js_1.legacyHarTasksInitialize)(e), e;
  }
  createProjectModel(i) {
    const e = i.getProject(),
      t = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_APP_PLUGIN);
    return t || this._log.printErrorExit("FA_NO_SYSTEM_PLUGIN", [`${[e.getBuildFilePath()]}.ts`], [[`${[e.getBuildFilePath()]}.ts`]]), t.getProjectModel();
  }
  initPluginParam(i, e, t) {
    const a = this.createProjectModel(e),
      n = this.createModuleModel(a, e, t);
    return i.withProjectModel(a).withModuleModel(n).withPluginApiType(hap_extra_info_js_1.ApiType.FA).withRuntimeConfigFileName(common_const_js_1.CommonConst.CONFIG_JSON);
  }
  setUpModulePlugin(i, e) {
    i.doModuleInspection(), i.initModuleTaskService(true), i.initBuildOptionMap(e.getBuildProfilePath()), i.initTargetDepends(), i.initModuleCommonTasks(), i.initModuleCommonTasksForDiffApiType(true);
  }
}
