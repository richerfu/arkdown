/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/project-task-service.js
 */
import util_1 from "util";
import * as hvigor_1 from "@ohos/hvigor";
import * as task_util_1 from "@ohos/hvigor/src/base/util/task-util";
import * as node_config_path_info_1 from "@ohos/hvigor/src/common/node-config-path-info";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as project_path_info_iml_js_1 from "../../common/iml/project-path-info-iml";
import * as ohos_trace_js_1 from "../../common/trace/ohos-trace";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as runtime_type_enum_js_1 from "../../enum/runtime-type-enum";
import * as abstract_hap_module_plugin_js_1 from "../../plugin/common/abstract-hap-module-plugin";
import * as ohos_plugin_id_js_1 from "../../plugin/common/ohos-plugin-id";
import * as hsp_plugin_js_1 from "../../plugin/hsp-plugin";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as project_extra_info_service_js_1 from "../../project/project-extra-info-service";
import * as sdk_loader_js_1 from "../../sdk/sdk-loader";
import * as array_util_js_1 from "../../utils/array-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_service_js_1 from "./task-service";
import * as global_project_data_service_js_1 from "./global-project-data-service";
const ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET;
export class ProjectTaskService extends task_service_js_1.TaskService {
  constructor(e, t, r, o) {
    super(e, t, r, o), this._log = ohos_logger_js_1.OhosLogger.getLogger(ProjectTaskService.name), this._targetProduct = (0, find_target_product_js_1.findTargetProduct)(t);
    const i = project_extra_info_service_js_1.ProjectExtraInfoService.getProjectRuntimeOS(t) === runtime_type_enum_js_1.RuntimeTypeEnum.OpenHarmony,
      s = t.getCompileApiMetaByProduct(this._targetProduct.name);
    this._sdkInfo = sdk_loader_js_1.SdkLoader.getSdkInfo(s, i), this._pathInfo = new project_path_info_iml_js_1.ProjectPathInfoIml(t, this._targetProduct.name), this._productDataMap = new Map(), this._buildOption = build_mode_manager_js_1.buildOptionManager.getProductBuildOption(this._targetProduct.name), global_project_data_service_js_1.GlobalProjectDataService.getInstance().initGlobalProjectData(t), ohos_trace_js_1.ohosTrace.traceNativeCompiler(this._buildOption.nativeCompiler);
  }
  async setup() {
    var e, t, r;
    await this._sdkInfo.setup(), process.env.OHOS_SDK_NATIVE = this._sdkInfo.getSdkNativeDir();
    hvigor_1.MetricFactory.createMarkEvent("sdkVersion", null !== (e = this._sdkInfo.getToolchainsComponentVersion()) && undefined !== e ? e : "").setMarkType(hvigor_1.MarkEventType.OTHER);
    if (null === (r = null === (t = this._targetProduct.buildOption) || undefined === t ? undefined : t.strictMode) || undefined === r ? undefined : r.caseSensitiveCheck) {
      hvigor_1.MetricFactory.createMarkEvent("caseSensitiveCheckOn", "caseSensitiveCheck check is on").setMarkType(hvigor_1.MarkEventType.OTHER);
    } else {
      hvigor_1.MetricFactory.createMarkEvent("caseSensitiveCheckOff", "caseSensitiveCheck check is off").setMarkType(hvigor_1.MarkEventType.OTHER);
    }
  }
  getSdkInfo() {
    return this._sdkInfo;
  }
  refreshData() {
    this._productDataMap.clear(), this.initProductData(), this._targetProduct = (0, find_target_product_js_1.findTargetProduct)(this._projectModel), this._buildOption = build_mode_manager_js_1.buildOptionManager.getProductBuildOption(this._targetProduct.name);
  }
  initProductData() {
    const e = this.hvigorNode;
    null == e || e.getSubModules().forEach(e => {
      var t, r;
      const o = e.getName(),
        i = null !== (t = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN)) && undefined !== t ? t : e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN),
        s = [];
      if (undefined !== i && (i instanceof abstract_hap_module_plugin_js_1.AbstractHapModulePlugin || i instanceof hsp_plugin_js_1.HspPlugin)) {
        const e = null === (r = this._projectModel) || undefined === r ? undefined : r.getModuleProfileOpt(o),
          t = null == e ? undefined : e.targets,
          _ = i.getTaskService().getTargetDataSet(),
          a = new Set();
        _.forEach(e => {
          a.add(e[0].getTargetName());
          undefined !== (0, array_util_js_1.getElementFromArr)(t, e[0].getTargetName()) && (this.checkIsOhosTestTarget(e[0]), e[1] && s.push(e[0]));
        }), this.checkTargetIsUnknown(o, a, t), 0 !== s.length && this._productDataMap.set(o, s);
      }
    });
  }
  getProductDataMap() {
    return this._productDataMap;
  }
  getPathInfo() {
    return this._pathInfo;
  }
  getTargetProduct() {
    return this._targetProduct;
  }
  getBuildOption() {
    return this._buildOption;
  }
  getAppOutputFileName(e = false) {
    var t;
    const r = e ? "signed" : "unsigned",
      o = this._projectModel.getName(),
      i = null === (t = this._targetProduct.output) || undefined === t ? undefined : t.artifactName;
    return `${i ? `${i}${e ? "" : "-unsigned"}` : `${o}-${this._targetProduct.name}-${r}`}${build_directory_const_js_1.BuildArtifactExtension.DOT_APP}`;
  }
  getAllAppOutputFileName(e = false) {
    var t;
    const r = e ? "signed" : "unsigned",
      o = this._projectModel.getName(),
      i = null === (t = this._targetProduct.output) || undefined === t ? undefined : t.artifactName;
    return `${i ? `${i}-all${e ? "" : "-unsigned"}` : `${o}-${this._targetProduct.name}-all-${r}`}${build_directory_const_js_1.BuildArtifactExtension.DOT_APP}`;
  }
  checkIsOhosTestTarget(e) {
    if (e.getTargetName() !== ohosTestTargetName) return;
    const t = e.getModuleModel().getModule().getName();
    this._log.printErrorExit("OHOS_TEST_CAN_NOT_APPLY_TO_PRODUCT", [this._projectModel.getProfilePath()], [[t]]);
  }
  checkTargetIsUnknown(e, t, r) {
    if (r && !(0, task_util_1.getAlignTarget)()) for (const o of r) t.has(o.name) || this._log.printErrorExit("UNKNOWN_TARGET_IN_PROJECT_BUILD_PROFILE", [o.name, node_config_path_info_1.nodeConfigPathInfo.getNodeConfigPathByName(e)], [[util_1.format(o)]]);
  }
}
