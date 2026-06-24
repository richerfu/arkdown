/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/file/oh-package-loader.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as class_type_const_js_1 from "@ohos/hvigor/src/base/common/options/class-type-const";
import * as node_config_path_info_1 from "@ohos/hvigor/src/common/node-config-path-info";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../../../const/common-const";
import * as json_util_js_1 from "../../json-util";
import * as ohos_logger_js_1 from "../../log/ohos-logger";
import * as config_file_loader_js_1 from "../config-file-loader";
class OhPackageLoader {
  constructor() {
    this._log = ohos_logger_js_1.OhosLogger.getLogger("OhPackageLoader"), this.isUpdatedOhPackageInfo = false, this.isLoaded = false, this.isChangedParameterFile = false, this.OhPackagePathMap = new Map(), this.UpdatePathToOriginMap = new Map();
  }
  getDependenciesOpt(e) {
    var o;
    return this.saveOhPackagePathInfoToLoader(), null === (o = this.getOhPackageJsonObj(e)) || undefined === o ? undefined : o.dependencies;
  }
  clean() {
    this.isUpdatedOhPackageInfo = false, this.isLoaded = false, this.OhPackagePathMap = new Map(), this.UpdatePathToOriginMap = new Map(), this.isChangedParameterFile = false;
  }
  getDevDependenciesOpt(e) {
    var o;
    return this.saveOhPackagePathInfoToLoader(), null === (o = this.getOhPackageJsonObj(e)) || undefined === o ? undefined : o.devDependencies;
  }
  getDynamicDependenciesOpt(e) {
    var o;
    return this.saveOhPackagePathInfoToLoader(), null === (o = this.getOhPackageJsonObj(e)) || undefined === o ? undefined : o.dynamicDependencies;
  }
  getOhPackageJsonObj(e) {
    var o;
    return config_file_loader_js_1.configFileLoader.getConfigFileJson(null !== (o = this.UpdatePathToOriginMap.get(e)) && undefined !== o ? o : e);
  }
  setDependenciesOpt(e, o) {
    this.validateDependency(o), this.saveOhPackagePathInfoToLoader();
    const t = config_file_loader_js_1.configFileLoader.getConfigFileJson(e);
    t.dependencies = o, config_file_loader_js_1.configFileLoader.setConfigFileJson(e, t), this.isUpdatedOhPackageInfo = true, hvigor_1.hvigorCore.getCache(common_const_js_1.CommonConst.DEP_NODE).clear();
  }
  setDevDependenciesOpt(e, o) {
    this.validateDependency(o), this.saveOhPackagePathInfoToLoader();
    const t = config_file_loader_js_1.configFileLoader.getConfigFileJson(e);
    t.devDependencies = o, config_file_loader_js_1.configFileLoader.setConfigFileJson(e, t), this.isUpdatedOhPackageInfo = true, hvigor_1.hvigorCore.getCache(common_const_js_1.CommonConst.DEP_NODE).clear();
  }
  setDynamicDependenciesOpt(e, o) {
    this.validateDependency(o), this.saveOhPackagePathInfoToLoader();
    const t = config_file_loader_js_1.configFileLoader.getConfigFileJson(e);
    t.dynamicDependencies = o, config_file_loader_js_1.configFileLoader.setConfigFileJson(e, t), this.isUpdatedOhPackageInfo = true, hvigor_1.hvigorCore.getCache(common_const_js_1.CommonConst.DEP_NODE).clear();
  }
  getVersion(e) {
    return this.saveOhPackagePathInfoToLoader(), config_file_loader_js_1.configFileLoader.getConfigFileJson(e).version;
  }
  setVersion(e, o) {
    "string" != typeof o && this._log.printErrorExit("VERSION_IS_NOT_A_STRING", [o]), this.saveOhPackagePathInfoToLoader();
    const t = config_file_loader_js_1.configFileLoader.getConfigFileJson(e);
    t.version = o, config_file_loader_js_1.configFileLoader.setConfigFileJson(e, t), this.isUpdatedOhPackageInfo = true;
  }
  shouldDoOhpmInstall() {
    return this.isUpdatedOhPackageInfo || this.isChangedParameterFile || node_config_path_info_1.nodeConfigPathInfo.getIsModuleChanged() || "true" === hvigor_1.hvigor.getParameterInternal().getExtParamInternal(common_const_js_1.CommonConst.ENFORCE_OHPM);
  }
  loadUpdatedOhPackageToDisk(e, o) {
    this._log.debug("start to load updatedOhPackageInfo to the disk"), this.saveOhPackagePathInfoToLoader();
    const t = path_1.resolve(hvigor_1.PathUtil.getHvigorCacheDir(), common_const_js_1.CommonConst.DEPENDENCYMAP, e),
      a = path_1.resolve(t, `${common_const_js_1.CommonConst.DEPENDENCYMAP}.json5`);
    fs_extra_1.existsSync(t) || fs_extra_1.mkdirSync(t, {
      recursive: true
    });
    const n = {
        basePath: a,
        targetName: e || undefined,
        rootDependency: `./${common_const_js_1.CommonConst.OH_PACKAGE_JSON5}`,
        dependencyMap: {},
        modules: []
      },
      s = new Set();
    hvigor_1.hvigor.getAllNodesInternal().forEach(e => {
      let i;
      i = e.getNodeNameInternal() === hvigor_1.hvigor.getRootNodeInternal().getNodeNameInternal() && e.classKind === class_type_const_js_1.ClassTypeConst.HVIGOR_PROJECT ? this.processProjectDependencyMap(e, t, o) : this.processModuleDependencyMap(e, t, a, s, n), this.updateOhPackagePathMapping(i);
    }), this.cleanUpRedundantModules(t, s);
    this.isFileContentEqual(a, n) && fs_extra_1.writeFileSync(a, JSON.stringify(n)), this._log.debug("load to the disk finished");
  }
  updateOhPackagePathMapping(e) {
    this.OhPackagePathMap.set(e.originPath, e.mapPath), this.UpdatePathToOriginMap.set(e.mapPath, e.originPath);
  }
  processProjectDependencyMap(e, o, t) {
    const a = path_1.resolve(e.getNodePathInternal(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5),
      n = config_file_loader_js_1.configFileLoader.getConfigFileJson(a);
    n.parameterFile = t;
    const s = path_1.resolve(o, common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
    return this.isFileContentEqual(s, n) && fs_extra_1.writeFileSync(s, JSON.stringify(n)), {
      originPath: a,
      mapPath: s
    };
  }
  processModuleDependencyMap(e, o, t, a, n) {
    const s = e.getNodeNameInternal(),
      i = e.getNodePathInternal(),
      r = path_1.resolve(i, common_const_js_1.CommonConst.OH_PACKAGE_JSON5),
      _ = path_1.resolve(o, s),
      d = path_1.resolve(_, common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
    a.add(s), fs_extra_1.existsSync(_) || fs_extra_1.mkdirSync(_, {
      recursive: true
    });
    const c = config_file_loader_js_1.configFileLoader.getConfigFileJson(r);
    return this.isFileContentEqual(d, c) && fs_extra_1.writeFileSync(d, JSON.stringify(c)), n.dependencyMap[s] = `./${s}/${common_const_js_1.CommonConst.OH_PACKAGE_JSON5}`, n.modules.push({
      name: s,
      srcPath: path_1.relative(t, i)
    }), {
      originPath: r,
      mapPath: d
    };
  }
  isFileContentEqual(e, o) {
    if (!fs_extra_1.existsSync(e)) return true;
    try {
      const t = fs_extra_1.readFileSync(e, "utf-8");
      return t !== JSON.stringify(o);
    } catch (e) {
      return true;
    }
  }
  cleanUpRedundantModules(e, o) {
    if (!fs_extra_1.existsSync(e)) return;
    const t = fs_extra_1.readdirSync(e);
    for (const a of t) {
      const t = path_1.resolve(e, a);
      if (fs_extra_1.statSync(t).isDirectory() && t.startsWith(path_1.resolve(e)) && !o.has(a)) try {
        fs_extra_1.removeSync(t), this._log.debug(`Removed redundant module directory: ${t}`);
      } catch (e) {
        this._log.warn(`Failed to remove redundant '${a}' module directory: ${t}`);
      }
    }
  }
  getNodeOhPackagePath(e) {
    var o;
    return this.isUpdatedOhPackageInfo && (null === (o = this.OhPackagePathMap) || undefined === o ? undefined : o.get(e)) || e;
  }
  saveOhPackagePathInfoToLoader() {
    this.isLoaded || (hvigor_1.hvigor.getAllNodesInternal().forEach(e => {
      const o = path_1.resolve(e.getNodePathInternal(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
      config_file_loader_js_1.configFileLoader.setConfigFileJson(o, json_util_js_1.JsonUtil.getJson5Obj(o));
    }), this.isLoaded = true);
  }
  validateDependency(e) {
    for (const o in e) "string" != typeof e[o] && this._log.printErrorExit("DEPENDENCY_VERSION_IS_NOT_A_STRING", [e[o]]);
  }
  getOriginPathFromUpdatePath(e) {
    return this.isUpdatedOhPackageInfo ? (e.forEach((o, t) => {
      o = o.includes(common_const_js_1.CommonConst.OH_PACKAGE_JSON5) ? o : path_1.resolve(o, common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
      const a = ohPackageLoader.UpdatePathToOriginMap.get(o);
      e[t] = a ? path_1.dirname(a) : path_1.dirname(o);
    }), e) : e = e.map(e => path_1.dirname(e));
  }
  setIsChangeParameterFile(e, o) {
    if ("string" != typeof e) return void this._log.printErrorExit("OHOS_ALIGN_TARGET_TYPE_ERROR", [path_1.resolve(hvigor_1.hvigor.getRootNodeInternal().getNodePathInternal(), "hvigor", "hvigor-config.json5")]);
    const t = path_1.resolve(hvigor_1.PathUtil.getHvigorCacheDir(), common_const_js_1.CommonConst.DEPENDENCYMAP, e, common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
    if (!fs_extra_1.existsSync(t) && o) return;
    const a = json_util_js_1.JsonUtil.getJson5Obj(t),
      n = null == a ? undefined : a.parameterFile;
    this.isChangedParameterFile = n !== o;
  }
  getOverrides(e) {
    var o;
    return this.saveOhPackagePathInfoToLoader(), null === (o = this.getOhPackageJsonObj(e)) || undefined === o ? undefined : o.overrides;
  }
  setOverrides(e, o) {
    this.validateDependency(o), this.saveOhPackagePathInfoToLoader();
    const t = config_file_loader_js_1.configFileLoader.getConfigFileJson(e);
    t.overrides = o, config_file_loader_js_1.configFileLoader.setConfigFileJson(e, t), this.isUpdatedOhPackageInfo = true;
  }
}
export const ohPackageLoader = new OhPackageLoader();
