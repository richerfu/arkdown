/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/legacy-plugin-builder.js
 */
import * as common_const_js_1 from "../../const/common-const";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_plugin_id_js_1 from "../common/ohos-plugin-id";
import * as legacy_hap_plugin_js_1 from "../legacy/legacy-hap-plugin";
import * as legacy_har_plugin_js_1 from "../legacy/legacy-har-plugin";
import * as core_plugin_builder_js_1 from "./core-plugin-builder";
export class LegacyPluginBuilder extends core_plugin_builder_js_1.CorePluginBuilder {
  constructor(e) {
    super(e), this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyPluginBuilder.name);
  }
  setUpModulePlugin(e) {
    e.doModuleInspection(), e.initModuleTaskService(true), e.initBuildOptionMap(this.module.getBuildProfilePath()), e.initTargetDepends(), e.initModuleCommonTasks(), e.initModuleCommonTasksForDiffApiType(true);
  }
  createFaHapPlugin() {
    return this.initPluginParam(new legacy_hap_plugin_js_1.LegacyHapPlugin(this.module), true);
  }
  createFaHarPlugin() {
    return this.initPluginParam(new legacy_har_plugin_js_1.LegacyHarPlugin(this.module), false);
  }
  initPluginParam(e, i) {
    const o = this.createProjectModel(),
      l = this.createModuleModel(o, i);
    return e.withProjectModel(o).withModuleModel(l).withPluginApiType(hap_extra_info_js_1.ApiType.FA).withRuntimeConfigFileName(common_const_js_1.CommonConst.CONFIG_JSON);
  }
  createProjectModel() {
    const e = this.module.getProject(),
      i = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_APP_PLUGIN);
    return i || this._log.printErrorExit("FA_NO_SYSTEM_PLUGIN", [`${[e.getBuildFilePath()]}.ts`], [[`${[e.getBuildFilePath()]}.ts`]]), i.getProjectModel();
  }
}
