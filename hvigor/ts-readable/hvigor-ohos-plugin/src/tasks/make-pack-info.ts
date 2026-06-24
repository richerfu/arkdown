/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/make-pack-info.js
 */
import * as wdk_1 from "@baize/wdk";
import * as file_util_js_1 from "../utils/file-util";
import * as form_util_js_1 from "../utils/form-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as merge_profile_js_1 from "./merge-profile";
import * as hvigor_1 from "@ohos/hvigor";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as common_const_js_1 from "../const/common-const";
import * as abstract_make_pack_info_js_1 from "./abstract-make-pack-info";
import * as abstract_merge_profile_js_1 from "./abstract-merge-profile";
var Task = task_names_js_1.TaskNames.Task;
export class MakePackInfo extends abstract_make_pack_info_js_1.AbstractMakePackInfo {
  declareInputs() {
    var e, o, t;
    const s = this.projectModel.getAppRes().getAppResOpt(),
      i = this.targetData.getProduct();
    return super.declareInputs().set("releaseType", this.sdkInfo.getReleaseType()).set(abstract_merge_profile_js_1.INPUT_KEY.projectConfigAppOpt, JSON.stringify(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.signingConfig)).set("integratedHsp", !!(null === (o = this.targetService.getBuildOption().arkOptions) || undefined === o ? undefined : o.integratedHsp)).set("appResOpt", JSON.stringify(s)).set("buildversionConfig", null !== (t = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_VERSION)) && undefined !== t ? t : "").set("productBuildOpt", JSON.stringify(i));
  }
  declareInputFiles() {
    const e = this.projectModel.getAppRes().getJsonPath(),
      o = this.targetService.getTargetData().getModuleSourceSetModel().getModuleTargetRes().getJsonPath(),
      t = MakePackInfo.getFormPathList(this.moduleModel, this.targetName),
      s = this.projectModel.getProfilePath(),
      i = new hvigor_1.FileSet().addEntry(e).addEntry(o).addEntry(s),
      r = form_util_js_1.FormUtil.getFormWidgetModuleByTargetName(this.moduleModel, this.targetName),
      n = r || this.moduleModel;
    return null == t || t.forEach(e => {
      const o = form_util_js_1.FormUtil.resolveFormConfigPath(n, this.targetName, e);
      file_util_js_1.FileUtil.fileExists(o) && i.addEntry(o);
    }), i;
  }
  constructor(e) {
    super(e, Task.MAKE_PACK_INFO), this.log = ohos_logger_js_1.OhosLogger.getLogger(MakePackInfo.name);
  }
  initTaskDepends() {
    this.declareDepends(merge_profile_js_1.MergeProfile.name);
  }
  doTaskAction() {
    var e, o, t, s, i, r, n, a, l, d, c, p;
    const u = this.projectModel.getAppRes().getAppResOpt(),
      g = (0, wdk_1.cloneDeep)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt),
      m = this.targetData.getProduct(),
      _ = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_VERSION),
      h = {
        code: null !== (t = null !== (o = null == g ? undefined : g.versionCode) && undefined !== o ? o : m.versionCode) && undefined !== t ? t : u.app.versionCode,
        name: null !== (i = null !== (s = null == g ? undefined : g.versionName) && undefined !== s ? s : m.versionName) && undefined !== i ? i : u.app.versionName,
        build: null !== (r = null != _ ? _ : m.buildVersion) && undefined !== r ? r : u.app.buildVersion,
        minCompatibleVersionCode: u.app.minCompatibleVersionCode
      };
    let f = null !== (a = null !== (n = null == g ? undefined : g.bundleName) && undefined !== n ? n : m.bundleName) && undefined !== a ? a : u.app.bundleName;
    const v = null === (l = this.targetService.getBuildOption().arkOptions) || undefined === l ? undefined : l.integratedHsp;
    this.moduleModel.isHspModule() && v && (f = "");
    const j = {
      bundleName: f,
      bundleType: null !== (p = null !== (c = null !== (d = null == g ? undefined : g.bundleType) && undefined !== d ? d : m.bundleType) && undefined !== c ? c : u.app.bundleType) && undefined !== p ? p : common_const_js_1.BundleType.APP,
      version: h
    };
    this.doMakePackInfo(this.log, j);
  }
  processExtensionAbilities(e, o, t) {
    if (undefined === e) return;
    const s = [];
    for (const i of e) {
      const e = {
        name: i.name,
        forms: this.processForms(MakePackInfo.processMetaData(i.metadata, o, t))
      };
      s.push(e);
    }
    return s;
  }
  static processMetaData(e, o, t) {
    if (undefined === e) return;
    const s = [];
    for (const i of e) {
      const e = MakePackInfo.processResourcePath(i.resource);
      if (i.name && i.name.indexOf("form") >= 0 && undefined !== e) {
        const i = o.getFormJsonObjByTargetName(t, e).forms;
        for (let e = 0; e < i.length; e++) s.push(i[e]);
      }
    }
    return s;
  }
  static getFormPathList(e, o) {
    const t = e.getJsonObjByTargetName(o).module.extensionAbilities;
    if (undefined === t) return;
    const s = [];
    for (const e of t) {
      if (undefined === e.metadata) return [];
      for (const o of e.metadata) {
        const e = MakePackInfo.processResourcePath(o.resource);
        e && s.push(e);
      }
    }
    return s;
  }
  static processResourcePath(e) {
    if (!e) return;
    const o = e.split(":");
    return 2 !== o.length || "$" !== o[0].charAt(0) || o[0].length < 2 ? undefined : `base/${o[0].substring(1)}/${o[1]}.json`;
  }
  getModuleObj(e, o, t, s) {
    var i, r, n, a, l;
    const d = o.getJsonObjByTargetName(s),
      c = this.projectModel.getAppRes().getAppResOpt(),
      p = (0, wdk_1.cloneDeep)(null === (i = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === i ? undefined : i.appOpt);
    let u;
    switch (null !== (a = null !== (n = null !== (r = null == p ? undefined : p.bundleType) && undefined !== r ? r : this.targetData.getProduct().bundleType) && undefined !== n ? n : c.app.bundleType) && undefined !== a ? a : common_const_js_1.BundleType.APP) {
      case common_const_js_1.BundleType.ATOMIC_SERVICE:
        u = true;
        break;
      case common_const_js_1.BundleType.APP:
        u = false;
        break;
      default:
        u = d.module.installationFree;
    }
    const g = {
        moduleType: d.module.type,
        installationFree: u,
        deliveryWithInstall: d.module.deliveryWithInstall,
        moduleName: d.module.name
      },
      m = {
        compatible: this.compatibleApiVersion,
        releaseType: this.sdkInfo.getReleaseType(),
        target: null !== (l = this.targetSdkVersion) && undefined !== l ? l : this.compileApiVersion
      };
    return {
      mainAbility: d.module.mainElement,
      deviceType: e,
      abilities: this.processAbilities(d.module.abilities),
      extensionAbilities: this.processExtensionAbilities(d.module.extensionAbilities, o, s),
      distro: g,
      apiVersion: m
    };
  }
  getPackageObj(e, o, t) {
    return [{
      deviceType: e.getTargetDeviceType(),
      moduleType: o.module.type,
      deliveryWithInstall: o.module.deliveryWithInstall,
      name: t
    }];
  }
}
