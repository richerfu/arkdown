/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/plugin-builder.js
 */
import * as common_const_js_1 from "../../const/common-const";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_plugin_id_js_1 from "../common/ohos-plugin-id";
import * as hap_plugin_js_1 from "../hap-plugin";
import * as har_plugin_js_1 from "../har-plugin";
import * as hsp_plugin_js_1 from "../hsp-plugin";
import * as core_plugin_builder_js_1 from "./core-plugin-builder";
export class PluginBuilder extends core_plugin_builder_js_1.CorePluginBuilder {
  constructor(e) {
    super(e), this._log = ohos_logger_js_1.OhosLogger.getLogger(PluginBuilder.name);
  }
  createStageHapPlugin() {
    return this.initPluginParam(new hap_plugin_js_1.HapPlugin(this.module), true);
  }
  createStageHspPlugin() {
    return this.initPluginParam(new hsp_plugin_js_1.HspPlugin(this.module), true);
  }
  createStageHarPlugin() {
    return this.initPluginParam(new har_plugin_js_1.HarPlugin(this.module), false);
  }
  initPluginParam(e, i) {
    const o = this.createProjectModel(),
      t = this.createModuleModel(o, i);
    return e.withProjectModel(o).withModuleModel(t).withPluginApiType(hap_extra_info_js_1.ApiType.STAGE).withRuntimeConfigFileName(common_const_js_1.CommonConst.MODULE_JSON5);
  }
  setUpModulePlugin(e) {
    e.doModuleInspection(), e.initModuleTaskService(false), e.initBuildOptionMap(this.module.getBuildProfilePath()), e.initTargetDepends(), e.initModuleCommonTasksForDiffApiType(false);
  }
  createProjectModel() {
    const e = this.module.getProject(),
      i = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_APP_PLUGIN);
    return i || this._log.printErrorExit("STAGE_NO_SYSTEM_PLUGIN", [`${[e.getBuildFilePath()]}.ts`], [[`${[e.getBuildFilePath()]}.ts`]]), i.getProjectModel();
  }
}
