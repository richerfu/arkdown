/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract-merge-profile.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
import * as har_target_util_js_1 from "../utils/har-target-util";
import * as inject_util_js_1 from "../utils/inject-util";
import * as cmake_util_js_1 from "../utils/cmake/cmake-util";
export const INPUT_KEY = {
  isHarModule: "isHarModule",
  bundleName: "bundleName",
  targetSdkVersion: "targetSdkVersion",
  compatibleSdkVersion: "compatibleSdkVersion",
  multiProjects: "multiProjects",
  releaseType: "releaseType",
  buildRoot: "buildRoot",
  isDebug: "isDebug",
  asanEnable: "asanEnable",
  tsanEnable: "tsanEnable",
  hwasanEnable: "hwasanEnable",
  ubsanEnable: "ubsanEnable",
  projectConfigAppOpt: "projectConfigAppOpt",
  vendor: "vendor",
  buildProfileAbilities: "buildProfileAbilities",
  appEnvironments: "appEnvironments",
  toolChainSdkVersion: "toolChainSdkVersion",
  moduleJsonOpt: "moduleJsonOpt",
  appJsonOpt: "appJsonOpt"
};
export class AbstractMergeProfile extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e, t) {
    var s, o, r, i;
    super(e, t);
    const a = (0, wdk_1.cloneDeep)(null === (s = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === s ? undefined : s.appOpt);
    this._moduleTargetData = this.targetService.getTargetData(), this._projectModel = this.service.getProjectModel(), this._bundleName = null !== (o = null == a ? undefined : a.bundleName) && undefined !== o ? o : this._moduleTargetData.getProduct().bundleName, this._vendor = null !== (r = null == a ? undefined : a.vendor) && undefined !== r ? r : this._moduleTargetData.getProduct().vendor, this._targetName = this._moduleTargetData.getTargetName(), this._pathInfo = this._moduleTargetData.getPathInfo(), this._buildRoot = this._pathInfo.getBuildRoot();
    const n = cmake_util_js_1.CmakeUtil.getCMakeArguments(null === (i = e.getBuildOption().externalNativeOptions) || undefined === i ? undefined : i.arguments);
    this.asanEnable = "true" === hvigor_1.hvigorCore.getExtraConfig().get("ohos-debug-asan") || n.includes("-DOHOS_ENABLE_ASAN=ON"), this.tsanEnable = "true" === hvigor_1.hvigorCore.getExtraConfig().get("ohos-enable-tsan") || n.includes("-DOHOS_ENABLE_TSAN=ON"), this.hwAsanEnable = "true" === hvigor_1.hvigorCore.getExtraConfig().get("ohos-enable-hwasan") || n.includes("-DOHOS_ENABLE_HWASAN=ON"), this.ubsanEnable = "true" === hvigor_1.hvigorCore.getExtraConfig().get("ohos-enable-ubsan") || n.includes("-DOHOS_ENABLE_UBSAN=ON"), this.appEnvironments = hvigor_1.hvigorCore.getParameter().getExtParamInternal("ohos-app-environment");
  }
  mergeDependsLibs(e) {
    const t = this.service.getDependencyRootPaths(),
      s = this.service.getHarModuleDependenciesWithRootPath(),
      o = this.service.getHarModuleDependencyPaths(),
      r = this.service.getHspModuleDependencyPaths();
    return t.map(t => {
      const i = inject_util_js_1.InjectUtil.getBuildCacheParentDir(t, path_1.join(hvigor_1.hvigorCore.getProject().getName(), t.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length)));
      if (r.includes(t)) {
        const t = path_1.resolve(i, this._buildRoot, this.targetData.getProduct().name, build_directory_const_js_1.BuildDirConst.INTERMEDIATES, build_directory_const_js_1.BuildDirConst.INTERMEDIATES_MERGE_PROFILE, this._targetName, e);
        return fs_extra_1.pathExistsSync(t) ? t : path_1.resolve(i, this._buildRoot, this.targetData.getProduct().name, build_directory_const_js_1.BuildDirConst.INTERMEDIATES, build_directory_const_js_1.BuildDirConst.INTERMEDIATES_MERGE_PROFILE, common_const_js_1.DefaultTargetConst.DEFAULT_TARGET, e);
      }
      return o.includes(t) ? this.getHarDenPath(s, t, i, e) : path_1.resolve(t, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, e);
    });
  }
  getHarDenPath(e, t, s, o) {
    var r;
    const i = e.get(t),
      a = path_1.resolve(s, this._buildRoot, this.targetData.getProduct().name, build_directory_const_js_1.BuildDirConst.INTERMEDIATES, build_directory_const_js_1.BuildDirConst.INTERMEDIATES_MERGE_PROFILE, common_const_js_1.DefaultTargetConst.DEFAULT_TARGET, o);
    if (!i) return a;
    const n = null !== (r = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).get(i[0])) && undefined !== r ? r : "default",
      _ = path_1.resolve(s, this._buildRoot, this.targetData.getProduct().name, build_directory_const_js_1.BuildDirConst.INTERMEDIATES, build_directory_const_js_1.BuildDirConst.INTERMEDIATES_MERGE_PROFILE, n, o);
    return fs_extra_1.pathExistsSync(_) ? _ : a;
  }
  apiTransform(e) {
    if (e.version <= sdk_const_js_1.ApiVersion.API_VERSION_9) return e.version;
    let t = "";
    const s = e.api.split(".");
    for (const e of s) t += e.padStart(2, "0");
    t += `${e.version}`.padStart(3, "0");
    let o = 0;
    for (let e = 0; e < t.length; e++) if (0 !== (0, wdk_1.parseInt)(t[e])) {
      o = e;
      break;
    }
    return t = t.substring(o), (0, wdk_1.parseInt)(t);
  }
  declareInputs() {
    const e = new Map();
    this._bundleName && e.set(INPUT_KEY.bundleName, this._bundleName), this._vendor && e.set(INPUT_KEY.vendor, this._vendor), e.set(INPUT_KEY.targetSdkVersion, this.compileApiVersion), e.set(INPUT_KEY.compatibleSdkVersion, this.compatibleApiVersion), e.set(INPUT_KEY.releaseType, this.sdkInfo.getReleaseType()), e.set(INPUT_KEY.buildRoot, this._buildRoot), e.set(INPUT_KEY.isDebug, this.targetService.isDebug()), e.set(INPUT_KEY.asanEnable, this.asanEnable), e.set(INPUT_KEY.tsanEnable, this.tsanEnable), e.set(INPUT_KEY.hwasanEnable, this.hwAsanEnable), e.set(INPUT_KEY.ubsanEnable, this.ubsanEnable), this.appEnvironments && e.set(INPUT_KEY.appEnvironments, this.appEnvironments);
    const t = this.moduleModel.getJsonProfileOptByTargetName(this.targetName, this.isFaMode);
    return e.set(INPUT_KEY.moduleJsonOpt, JSON.stringify(t)), e;
  }
  parseToAppEnvironments(e) {
    const t = [];
    if (undefined === e) return t;
    const s = e.replace(/^\s*{\s*(.*)\s*}\s*$/, "$1"),
      o = /(\w+):([^,]+)/g;
    let r;
    for (; null !== (r = o.exec(s));) {
      const e = {
        name: r[1],
        value: r[2].trim()
      };
      t.push(e);
    }
    return t;
  }
}
