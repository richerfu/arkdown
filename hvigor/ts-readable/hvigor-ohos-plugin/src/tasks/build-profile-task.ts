/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/build-profile-task.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as common_const_js_1 from "../const/common-const";
import * as build_profile_utils_js_1 from "../utils/build-profile-utils";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_util_js_1 from "../utils/task-util";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
var __decorate = this && this.__decorate || function (e, o, t, i) {
  var r,
    l = arguments.length,
    a = l < 3 ? o : null === i ? i = Object.getOwnPropertyDescriptor(o, t) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, o, t, i);else for (var s = e.length - 1; s >= 0; s--) (r = e[s]) && (a = (l < 3 ? r(a) : l > 3 ? r(o, t, a) : r(o, t)) || a);
  return l > 3 && a && Object.defineProperty(o, t, a), a;
};
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class BuildProfileTask extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, CommonTask.CREATE_BUILD_PROFILE), this._log = ohos_logger_js_1.OhosLogger.getLogger(BuildProfileTask.name), this.buildTaskService = e, this.initDefaultData();
  }
  initDefaultData() {
    var e, o, t, i, r, l, a, s, u, n;
    const d = this.projectModel.getAppRes().getAppResOpt(),
      _ = (0, wdk_1.cloneDeep)(this.projectOhosConfigAppOpt),
      p = this.targetData.getProduct();
    this.buildProfileData = {
      BUNDLE_NAME: null !== (o = null !== (e = null == _ ? undefined : _.bundleName) && undefined !== e ? e : this.targetData.getProduct().bundleName) && undefined !== o ? o : this.projectModel.getDefaultBundleName(),
      BUNDLE_TYPE: null !== (r = null !== (i = null !== (t = null == _ ? undefined : _.bundleType) && undefined !== t ? t : p.bundleType) && undefined !== i ? i : d.app.bundleType) && undefined !== r ? r : common_const_js_1.BundleType.APP,
      VERSION_CODE: null !== (a = null !== (l = null == _ ? undefined : _.versionCode) && undefined !== l ? l : p.versionCode) && undefined !== a ? a : d.app.versionCode,
      VERSION_NAME: null !== (u = null !== (s = null == _ ? undefined : _.versionName) && undefined !== s ? s : p.versionName) && undefined !== u ? u : d.app.versionName,
      TARGET_NAME: this.targetName,
      PRODUCT_NAME: this.targetData.getProduct().name,
      BUILD_MODE_NAME: this.buildModeName,
      DEBUG: this.buildTaskService.isDebug()
    };
    const g = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_VERSION),
      h = null !== (n = null != g ? g : p.buildVersion) && undefined !== n ? n : d.app.buildVersion;
    undefined !== h && (this.buildProfileData.BUILD_VERSION = h);
  }
  get buildMode() {
    return this.targetService.getBuildMode();
  }
  get buildModeName() {
    return (0, task_util_js_1.getBuildModeName)();
  }
  get buildProfileFields() {
    return this.targetService.getBuildProfileFields();
  }
  get projectOhosConfigAppOpt() {
    var e;
    return null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt;
  }
  declareInputs() {
    var e;
    return super.declareInputs().set("buildversionConfig", null !== (e = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_VERSION)) && undefined !== e ? e : "");
  }
  get appResJsonPath() {
    return this.service.getProjectModel().getAppRes().getJsonPath();
  }
  get projectProfilePath() {
    return this.projectModel.getProfilePath();
  }
  get buildProfilePath() {
    return this.pathInfo.getGenerateBuildProfilePath(this.targetService.getTargetData().getTargetName());
  }
  get bundleName() {
    var e;
    return null === (e = this.buildProfileData) || undefined === e ? undefined : e.BUNDLE_NAME;
  }
  get bundleType() {
    var e;
    return null === (e = this.buildProfileData) || undefined === e ? undefined : e.BUNDLE_TYPE;
  }
  get versionCode() {
    var e;
    return null === (e = this.buildProfileData) || undefined === e ? undefined : e.VERSION_CODE;
  }
  get versionName() {
    var e;
    return null === (e = this.buildProfileData) || undefined === e ? undefined : e.VERSION_NAME;
  }
  get productName() {
    var e;
    return null === (e = this.buildProfileData) || undefined === e ? undefined : e.PRODUCT_NAME;
  }
  get debug() {
    var e;
    return null === (e = this.buildProfileData) || undefined === e ? undefined : e.DEBUG;
  }
  initTaskDepends() {}
  doTaskAction() {
    const e = this.buildProfileFields ? {
      ...this.buildProfileData,
      ...this.buildProfileFields
    } : {
      ...this.buildProfileData
    };
    hvigor_1.LocalFileWriter.getInstance().writeStr(this.buildProfilePath, (0, build_profile_utils_js_1.normalizedFileData)(e, this.targetData, this._log));
  }
}
__decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "buildMode", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "buildModeName", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "buildProfileFields", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "projectOhosConfigAppOpt", null), __decorate([(0, hvigor_1.InputFile)()], BuildProfileTask.prototype, "appResJsonPath", null), __decorate([(0, hvigor_1.InputFile)()], BuildProfileTask.prototype, "projectProfilePath", null), __decorate([(0, hvigor_1.OutputFile)()], BuildProfileTask.prototype, "buildProfilePath", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "bundleName", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "bundleType", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "versionCode", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "versionName", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "productName", null), __decorate([(0, hvigor_1.Input)()], BuildProfileTask.prototype, "debug", null);
