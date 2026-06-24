/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/global-project-data-service.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as common_const_js_1 from "../../const/common-const";
import * as file_util_js_1 from "../../utils/file-util";
import * as integrated_hsp_utils_js_1 from "../../utils/integrated-hsp-utils";
import * as json_util_js_1 from "../../utils/json-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_test_util_js_1 from "../../utils/ohos-test-util";
import * as sign_util_js_1 from "../sign/sign-util";
export class GlobalProjectDataService {
  constructor() {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(GlobalProjectDataService.name), this.compileProjectModel = {}, this.allModulePaths = [], this.projectPkgJsonFileHash = "", this.allModuleNameHash = "", this.rootPathSet = new Set(), this.isEmptyTargetSdkVersionCheck = false, this.buildModeInfo = {
      ifThrowError: false,
      buildModeName: "",
      buildProfilePath: ""
    }, this.moduleRemoteHspSet = new Set(), this.remoteHspCache = {
      signingConfig: {},
      signedRemoteHsps: new Map()
    };
  }
  static getInstance() {
    return GlobalProjectDataService.INSTANCE;
  }
  initGlobalProjectData(e) {
    this.rootPathSet = new Set(), this.allModulePaths = [], this.compileProjectModel = this.initCompileProjectModel(e), this.allModuleNameHash = this.initAllModuleNameHash(e), this.projectRemoteHspPromise = undefined, this.moduleRemoteHspSet = new Set(), this.initRemoteHspCache(e), this.integratedHspUtils = new integrated_hsp_utils_js_1.IntegratedHspUtils(e), this.buildModeInfo = {
      ifThrowError: false,
      buildModeName: "",
      buildProfilePath: ""
    }, this.isEmptyTargetSdkVersionCheck = false;
  }
  getIsEmptyTargetSdkVersionCheck() {
    return this.isEmptyTargetSdkVersionCheck;
  }
  setIsEmptyTargetSdkVersionCheck() {
    this.isEmptyTargetSdkVersionCheck = true;
  }
  getCompileProjectModel() {
    return this.compileProjectModel;
  }
  setProjectPkgJsonFileHash(e) {
    this.projectPkgJsonFileHash = e;
  }
  getProjectPkgJsonFileHash() {
    return this.projectPkgJsonFileHash;
  }
  getAllModuleNameHash() {
    return this.allModuleNameHash;
  }
  setProjectRemoteHspPromise(e) {
    this.projectRemoteHspPromise = e;
  }
  getProjectRemoteHspPromise() {
    return this.projectRemoteHspPromise;
  }
  setModuleRemoteHspPromise(e) {
    this.moduleRemoteHspSet.add(e);
  }
  getModuleRemoteHspPromise() {
    return this.moduleRemoteHspSet;
  }
  setNeedSignedRemoteHspMap(e, t) {
    this.remoteHspCache.signedRemoteHsps.set(e, t);
  }
  getRemoteHspCache() {
    return this.remoteHspCache;
  }
  getBuildModeInfo() {
    return this.buildModeInfo;
  }
  setBuildModeInfo(e) {
    return this.buildModeInfo = e;
  }
  needSignedRemoteHsp(e, t) {
    if (this.remoteHspCache.signedRemoteHsps.has(e)) {
      const o = this.remoteHspCache.signedRemoteHsps.get(e);
      return !!(null == o ? undefined : o.moduleName) && o.moduleName === t;
    }
    return true;
  }
  changeRemoteHspSigned(e) {
    for (const t of this.remoteHspCache.signedRemoteHsps.values()) t.signedHspPath === e && (t.isSigned = true);
  }
  allRemoteHspSigned() {
    for (const e of this.remoteHspCache.signedRemoteHsps.values()) if (!e.isSigned) return false;
    return true;
  }
  getIntegratedHspUtils() {
    return this.integratedHspUtils;
  }
  getAllModulePaths() {
    return this.allModulePaths;
  }
  initCompileProjectModel(e) {
    const t = {};
    return e.getAllModules().forEach(o => {
      var s, i;
      this.rootPathSet.add(o.getBelongProjectPath());
      const r = o.getProjectDir();
      this.allModulePaths.push(r);
      for (const [e, i] of o.getAllTargetSourceSet()) {
        const a = null !== (s = i.getTargetSourceSetModuleName()) && undefined !== s ? s : o.getName(),
          l = i.getSourceSetRoot();
        if ("ohosTest" === e && (0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(l)) {
          const e = (0, ohos_test_util_js_1.getOhosTestIntermediatesEtsPath)(r);
          t[e] = {
            moduleName: a,
            modulePkgPath: r,
            belongProjectPath: o.getBelongProjectPath()
          };
        }
        t[l] = {
          moduleName: a,
          modulePkgPath: r,
          belongProjectPath: o.getBelongProjectPath()
        };
      }
      t[r] = {
        moduleName: o.getName(),
        modulePkgPath: r,
        belongProjectPath: o.getBelongProjectPath()
      };
      let a = null !== (i = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_CACHE_DIR)) && undefined !== i ? i : hvigor_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.CommonConst.BUILD_DIR_KEY);
      if (a) {
        fs_extra_1.ensureDirSync(a), a = fs_extra_1.realpathSync.native(a), this.rootPathSet.add(path_1.resolve(a, e.getName()));
        const s = path_1.join(a, path_1.join(hvigor_1.hvigorCore.getProject().getName(), r.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length)));
        t[s] = {
          moduleName: o.getName(),
          modulePkgPath: o.getProjectDir(),
          belongProjectPath: o.getBelongProjectPath(),
          isBuildCacheDir: true
        }, this.allModulePaths.push(s);
      }
    }), t[e.getProjectDir()] = {
      moduleName: e.getName(),
      modulePkgPath: e.getProjectDir(),
      belongProjectPath: e.getProjectDir()
    }, t;
  }
  initProjectPkgJsonFileHash(e) {
    var t;
    const o = e.isOhpmProject();
    let s = e.getAllModules().map(e => {
      var t;
      const s = o ? e.getOhPackageJson5Path() : e.getPackageJsonPath();
      return null !== (t = file_util_js_1.FileUtil.readFile(s)) && undefined !== t ? t : "";
    }).join("");
    const i = o ? e.getOhPackageJson5Path() : e.getPackageJsonPath();
    return s += null !== (t = file_util_js_1.FileUtil.readFile(i)) && undefined !== t ? t : "", (0, hvigor_1.hash)(s);
  }
  initAllModuleNameHash(e) {
    let t = e.getAllModules().map(e => {
      var t;
      let o = "";
      for (const [s, i] of e.getAllTargetSourceSet()) o += null !== (t = i.getTargetSourceSetModuleName()) && undefined !== t ? t : e.getName();
      return o;
    }).join("");
    return t += e.getName(), (0, hvigor_1.hash)(t);
  }
  getRootPathSet() {
    return this.rootPathSet;
  }
  resetRemoteHspCache() {
    this.remoteHspCache = {
      signingConfig: {},
      signedRemoteHsps: new Map()
    };
  }
  initRemoteHspCache(e) {
    this.resetRemoteHspCache();
    const t = (0, find_target_product_js_1.findTargetProduct)(e);
    if (!t) return void this._log.debug("No product found, initRemoteHspCache failed.");
    const o = (0, sign_util_js_1.getSigningConfig)(e),
      s = e.getCacheRemoteHspPath(t.name),
      i = path_1.resolve(s, common_const_js_1.CommonConst.REMOTE_HSP_CACHE);
    if (!o) return void this._log.debug("No signingConfig found, initRemoteHspCache failed.");
    if (this.remoteHspCache.signingConfig = o, !fs_extra_1.existsSync(i)) return void this._log.debug(`${i} not exists.`);
    const r = json_util_js_1.JsonUtil.getJson5Obj(i);
    if (!(0, wdk_1.isEqual)(r.signingConfig, o)) return void this._log.debug("The signature of the remote HSP package is not equal.");
    if (!r.signedRemoteHsps) return void this._log.debug("The signedRemoteHsps of the remote HSP package is not exists.");
    const a = fs_extra_1.readdirSync(s),
      l = new Map();
    Object.keys(r.signedRemoteHsps).forEach(e => {
      const t = r.signedRemoteHsps[e];
      a.includes(e) && (null == t ? undefined : t.signedHspPath) && fs_extra_1.existsSync(t.signedHspPath) && l.set(e, t);
    }), this.remoteHspCache.signedRemoteHsps = l;
  }
}
GlobalProjectDataService.INSTANCE = new GlobalProjectDataService();
