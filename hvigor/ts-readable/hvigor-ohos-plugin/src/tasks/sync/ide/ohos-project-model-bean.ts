/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sync/ide/ohos-project-model-bean.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as project_ohos_config_manager_js_1 from "../../../common/global/project-ohos-config-manager";
import * as common_const_js_1 from "../../../const/common-const";
import * as project_model_impl_js_1 from "../../../model/project/project-model-impl";
import * as ohos_plugin_id_js_1 from "../../../plugin/common/ohos-plugin-id";
import * as project_build_profile_loader_js_1 from "../../../utils/loader/file/project-build-profile-loader";
import * as task_util_js_1 from "../../../utils/task-util";
import * as global_project_data_service_js_1 from "../../service/global-project-data-service";
export class OhosProjectModelBean {
  constructor(e) {
    this.modelId = "ohos-project", this._project = e;
    const o = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_APP_PLUGIN);
    this._projectTaskService = o.getTaskService();
  }
  computeCommonInfo(e) {
    var o, t, r, s, i;
    const _ = (0, wdk_1.cloneDeep)(null === (o = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === o ? undefined : o.appOpt),
      l = this._projectTaskService.getTargetProduct(),
      c = this._projectTaskService.getPathInfo(),
      a = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_VERSION);
    e.set("SELECT_PRODUCT_NAME", l.name), e.set("MODULE_BUILD_DIR", c.getProjectBuildPath());
    let n = null !== (t = null == _ ? undefined : _.bundleName) && undefined !== t ? t : l.bundleName;
    if (undefined === n && (n = this._projectTaskService.getProjectModel().getDefaultBundleName()), e.set("BUNDLE_NAME", n), this._projectTaskService.getProjectModel() instanceof project_model_impl_js_1.ProjectModelImpl) {
      const o = null !== (r = null == _ ? undefined : _.versionCode) && undefined !== r ? r : this._projectTaskService.getProjectModel().getAppRes().getAppResOpt().app.versionCode,
        t = null !== (s = null == _ ? undefined : _.versionName) && undefined !== s ? s : this._projectTaskService.getProjectModel().getAppRes().getAppResOpt().app.versionName,
        c = null !== (i = null != a ? a : l.buildVersion) && undefined !== i ? i : this._projectTaskService.getProjectModel().getAppRes().getAppResOpt().app.buildVersion;
      o && e.set("VERSION_CODE", o), t && e.set("VERSION_NAME", t), c && e.set("BUILD_VERSION", c);
    }
  }
  computePathInfo(e) {
    const o = {
      OUTPUT_PATH: this._projectTaskService.getPathInfo().getProjectOutputPath()
    };
    e.set("BUILD_PATH", o);
  }
  computeModulesInfo(e) {
    const o = (0, wdk_1.cloneDeep)(project_build_profile_loader_js_1.projectBuildProfileLoader.getBuildProfile().modules);
    o.forEach(e => {
      var o, t, r;
      e.srcPath = path_1.resolve(this._project.getNodeDir(), e.srcPath), e.belongProjectPath = null === (r = null === (t = null === (o = this._projectTaskService.getProjectModel()) || undefined === o ? undefined : o.getSubModuleModels()) || undefined === t ? undefined : t.get(e.name)) || undefined === r ? undefined : r.getBelongProjectPath();
    }), e.set("MODULES", o);
  }
  computeProfileOpt(e) {
    e.set("PROFILE_OPT", this._projectTaskService.getProjectModel().getProfileOpt());
  }
  computeConfigProperties(e) {
    e.set("CONFIG_PROPERTIES", hvigor_1.hvigorCore.getParameter().getPropertiesInternal());
  }
  computeOverallProjectPaths(e) {
    e.set("OVERALL_PROJECT_PATHS", global_project_data_service_js_1.GlobalProjectDataService.getInstance().getRootPathSet());
  }
  computeBuildDirInfo(e) {
    var o;
    let t = null !== (o = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_CACHE_DIR)) && undefined !== o ? o : hvigor_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.CommonConst.BUILD_DIR_KEY);
    undefined !== t && fs_extra_1.existsSync(t) && (t = fs_extra_1.realpathSync.native(t)), e.set("BUILD_CACHE_DIR", t || "");
  }
  buildModel() {
    const e = new Map();
    return this.computeCommonInfo(e), this.computePathInfo(e), this.computeModulesInfo(e), this.computeProfileOpt(e), this.computeConfigProperties(e), this.computeOverallProjectPaths(e), this.computeBuildDirInfo(e), (0, hvigor_common_1.strMapToObj)(e);
  }
  taskInfo() {
    const e = [];
    return this._project.getAllTasksCA().forEach(o => {
      e.push((0, task_util_js_1.getTaskInfoFromTaskCA)(o));
    }), this._project.getAllTasks().forEach(o => {
      e.push((0, task_util_js_1.getTaskInfoFromTask)(o));
    }), e;
  }
}
