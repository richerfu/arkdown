/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/integrated-hsp-utils.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as wdk_1 from "@baize/wdk";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as common_const_1 from "../const/common-const";
import * as packing_tool_options_js_1 from "../builder/inner-java-command-builder/packing-tool-options";
import * as find_target_product_js_1 from "../common/find-target-product";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as json_util_js_1 from "./json-util";
import * as ohos_logger_1 from "./log/ohos-logger";
const log = ohos_logger_1.OhosLogger.getLogger("IntegratedHspUtils");
export class IntegratedHspUtils {
  constructor(e) {
    this.integratedHspCache = {
      bundleName: "",
      versionCode: "",
      buildVersion: "",
      integratedRemoteHsps: new Map()
    };
    const t = (0, find_target_product_js_1.findTargetProduct)(e);
    this.integratedHspCachePath = path_1.resolve(e.getCacheIntegratedHspPath(t.name), build_directory_const_js_1.BuildArtifactConst.INTEGRATED_HSP_CACHE), this.checkAppJson(e);
  }
  initIntegratedHspCache(e) {
    var t, o, s, n, r, i;
    const a = this.checkAppJson(e);
    if (null === a || !a.app) return;
    const d = hvigor_1.hvigorCore.getExtraConfig().get(common_const_1.CommonConst.BUILD_VERSION),
      p = (0, wdk_1.cloneDeep)(null === (t = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === t ? undefined : t.appOpt),
      l = (0, find_target_product_js_1.findTargetProduct)(e),
      g = null !== (s = null !== (o = null == p ? undefined : p.bundleName) && undefined !== o ? o : l.bundleName) && undefined !== s ? s : a.app.bundleName,
      u = null !== (r = null !== (n = null == p ? undefined : p.versionCode) && undefined !== n ? n : l.versionCode) && undefined !== r ? r : a.app.versionCode,
      c = null !== (i = null != d ? d : l.buildVersion) && undefined !== i ? i : a.app.buildVersion,
      _ = {
        bundleName: g,
        versionCode: null == u ? undefined : u.toString(),
        buildVersion: c,
        integratedRemoteHsps: new Map()
      };
    if (!fs_extra_1.existsSync(this.integratedHspCachePath)) return void (this.integratedHspCache = _);
    const h = json_util_js_1.JsonUtil.getJson5Obj(this.integratedHspCachePath),
      m = path_1.dirname(this.integratedHspCachePath),
      f = h.bundleName === g,
      C = h.versionCode === u.toString(),
      H = h.buildVersion === c;
    if (!(f && C && H)) return void (this.integratedHspCache = _);
    const v = fs_extra_1.readdirSync(path_1.dirname(this.integratedHspCachePath));
    Object.keys(h.integratedRemoteHsps).forEach(e => {
      const t = h.integratedRemoteHsps[e];
      v.includes(e) && (null == t ? undefined : t.hspPath) && fs_extra_1.existsSync(t.hspPath) && fs_extra_1.existsSync(path_1.resolve(m, t.hspDirName, t.hspFileName)) && _.integratedRemoteHsps.set(e, t);
    }), this.integratedHspCache = _;
  }
  getIntegratedHspCache() {
    return this.integratedHspCache;
  }
  checkAppJson(e) {
    if (e.isFaMode()) return null;
    const t = e.getAppRes().getAppResOpt();
    if (!t) {
      const t = path_1.resolve(e.getProjectDir(), common_const_1.CommonConst.APP_SCOPE, common_const_1.CommonConst.APP_CONFIG);
      log.printErrorExit("APPJSON5_NOT_FOUND", [t]);
    }
    return t;
  }
  isNeedIntegrated(e) {
    return !this.integratedHspCache.integratedRemoteHsps.has(e);
  }
  setIntegratedRemoteHsps(e) {
    this.integratedHspCache.integratedRemoteHsps.set(e.hspDirName, e);
  }
  changeIntegratedHspStatus(e) {
    for (const t of this.integratedHspCache.integratedRemoteHsps.values()) t.hspDirName === e && (t.isIntegratedHsp = true);
  }
  writeIntegratedHspCache() {
    if (0 !== this.integratedHspCache.integratedRemoteHsps.size) {
      for (const e of this.integratedHspCache.integratedRemoteHsps.values()) if (!e.isIntegratedHsp) return;
      fs_extra_1.existsSync(path_1.dirname(this.integratedHspCachePath)) || fs_extra_1.mkdirSync(path_1.dirname(this.integratedHspCachePath), {
        recursive: true
      }), fs_extra_1.writeJsonSync(this.integratedHspCachePath, {
        bundleName: this.integratedHspCache.bundleName,
        versionCode: this.integratedHspCache.versionCode,
        buildVersion: this.integratedHspCache.buildVersion,
        integratedRemoteHsps: Object.fromEntries(this.integratedHspCache.integratedRemoteHsps)
      });
    } else fs_extra_1.existsSync(path_1.dirname(this.integratedHspCachePath)) && fs_extra_1.removeSync(path_1.dirname(this.integratedHspCachePath));
  }
  resetIntegratedHspCache() {
    this.integratedHspCache = {
      bundleName: "",
      versionCode: "",
      buildVersion: "",
      integratedRemoteHsps: new Map()
    };
  }
}
export function generateIntegratedHspCommand(e, t, o, s) {
  const {
      bundleName: n,
      versionCode: r,
      buildVersion: i
    } = getAppInfoByProject(e),
    a = new packing_tool_options_js_1.PackingToolOptions();
  return a.addCalledJarFile(t), a.addMode("generalNormalize").addBundleName(n).addVersionCode(r.toString()).addInputList(o).addOutPath(s).addBuildVersion(i).build();
}
export function getAppInfoByProject(e) {
  var t, o, s, n, r, i;
  const a = e.getAppRes().getAppResOpt(),
    d = (0, wdk_1.cloneDeep)(null === (t = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === t ? undefined : t.appOpt),
    p = (0, find_target_product_js_1.findTargetProduct)(e),
    l = hvigor_1.hvigorCore.getExtraConfig().get(common_const_1.CommonConst.BUILD_VERSION);
  return {
    bundleName: null !== (s = null !== (o = null == d ? undefined : d.bundleName) && undefined !== o ? o : p.bundleName) && undefined !== s ? s : a.app.bundleName,
    versionCode: null !== (r = null !== (n = null == d ? undefined : d.versionCode) && undefined !== n ? n : p.versionCode) && undefined !== r ? r : a.app.versionCode,
    buildVersion: null !== (i = null != l ? l : p.buildVersion) && undefined !== i ? i : a.app.buildVersion
  };
}
