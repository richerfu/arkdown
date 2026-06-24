/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/dependency-manager.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import fs_1 from "fs";
import path_1 from "path";
import * as common_util_js_1 from "../../common/common-util";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as legacy_module_model_impl_js_1 from "../../model/module/legacy-module-model-impl";
import * as core_project_model_impl_js_1 from "../../model/project/core-project-model-impl";
import * as har_extend_info_js_1 from "../../tasks/har/har-extend-info";
import * as error_url_js_1 from "../../utils/error-url";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as base_package_resolver_js_1 from "../../utils/resolver/base-package-resolver";
import * as npm_package_resolver_js_1 from "../../utils/resolver/npm-package-resolver";
import * as ohpm_package_resolver_js_1 from "../../utils/resolver/ohpm-package-resolver";
import * as dependency_builder_js_1 from "./core/dependency-builder";
import * as dependency_interface_js_1 from "./core/dependency-interface";
import * as module_dependency_info_js_1 from "./module-dependency-info";
import * as legacy_dependency_manager_helper_js_1 from "./legacy-dependency-manager-helper";
import * as dependency_util_js_1 from "../../utils/dependency-util";
import * as copy_resources_util_js_1 from "../../utils/copy-resources-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as oh_modules_lock_loader_js_1 from "../../utils/loader/file/oh-modules-lock-loader";
import * as fse from "fs-extra";
const log = ohos_logger_js_1.OhosLogger.getLogger("ohos-dependency-manager"),
  solution = `The OpenHarmony npm package or module is not compatible with the current model. Use a compatible OpenHarmony npm package or module. For details about the stage and FA models, see ${error_url_js_1.ErrorUrl.ERROR_URL.modelDiff}.`;
export class DependencyManager {
  constructor(e, n, o) {
    this._allDependency = new Set(), this._model = n, this._projectModel = o, this._moduleName = n.getName(), this._isFaMode = e, this._isOhpmDependency = this.isOhpmDependency(), this.legacyDependencyManagerHelper = new legacy_dependency_manager_helper_js_1.LegacyDependencyManagerHelper(n.getName());
  }
  isOhpmDependency() {
    return this._projectModel ? this._projectModel.isOhpmProject() : this._model.isOhpmProject();
  }
  createModelDependencyInfo() {
    const e = (e, n, o) => n && e.npm.getDependencyEnum() !== dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES || o && e.npm.getDependencyType() !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP,
      n = this._isOhpmDependency ? this._model.getOhPackageJson5Path() : this._model.getPackageJsonPath(),
      {
        npmDependencies: o,
        moduleDependencyMap: t,
        modulePkgNode: s,
        projectPkgNode: d
      } = this.getDependenciesByFilter(this._model, n, e, e);
    return log.debug(`Module ${this._moduleName} Collected Dependency: ${o.map(e => e.getDependencyRootPath())}`), log.debug(`Module ${this._moduleName}'s total dependency: ${o.length}`), new module_dependency_info_js_1.ModuleDependencyInfo(s.npm, s, d, t, o);
  }
  collectModelOrProjectAllDependencyInfo() {
    const e = (e, n, o) => true,
      n = this._isOhpmDependency ? this._model.getOhPackageJson5Path() : this._model.getPackageJsonPath(),
      {
        npmDependencies: o
      } = this.getDependenciesByFilter(this._model, n, e, e);
    return o;
  }
  buildSelfDependency(e, n) {
    const o = e instanceof core_project_model_impl_js_1.CoreProjectModelImpl,
      t = o ? e.getProject() : e.getModule(),
      s = this.getFinalOhPkgJsonObj(n),
      d = this.getHarmonyDependencyType(n, s, n, t.getName());
    let r;
    o || (r = [path_1.resolve(e.getProjectDir(), "src/main/module.json5"), path_1.resolve(e.getProjectDir(), "src/main/module.json")].find(e => fs_1.existsSync(e)));
    const l = new dependency_builder_js_1.DependencyBuilder().setDependencyName(t.getName()).setDependencyRootPath(t.getNodeDir()).setPkgJsonPath(n).setPkgJsonObj(s).setIsLocal(true).setModule(t).setModuleJsonObj(this.getModuleJsonObj(r)).setIsModuleDependency(!o).setDependencyEnum(dependency_interface_js_1.DependencyEnum.DEPENDENCIES);
    return d && l.setDependencyType(d), l.buildDefaultModuleDependency();
  }
  collectAllDependencies() {
    const e = this._isOhpmDependency ? this._model.getOhPackageJson5Path() : this._model.getPackageJsonPath();
    return this.getDependenciesByFilter(this._model, e, undefined, (e, n, o) => o || e.npm.getDependencyEnum() !== dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES);
  }
  collectAllDependenciesForPkgInfo(e = false) {
    const n = this._isOhpmDependency ? this._model.getOhPackageJson5Path() : this._model.getPackageJsonPath(),
      o = (e, n) => {
        var o;
        return n || (null === (o = e.npm) || undefined === o ? undefined : o.getDependencyType()) !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP;
      };
    return e ? this.getDependenciesForPkgInfo(this._model, n, o, o) : this.getDependenciesForPkgInfo(this._model, n, o);
  }
  getDependenciesByFilter(e, n, o, t) {
    this._allDependency.clear();
    const s = [],
      d = new Map(),
      r = this.buildSelfDependency(e, n),
      l = {
        moduleName: this._moduleName,
        modulePkgJsonPath: n,
        children: [],
        npm: r,
        module: r
      };
    this.collectHarDependency(l), this.collectFromTree(l, s, d, o);
    return {
      npmDependencies: s,
      moduleDependencyMap: d,
      modulePkgNode: l,
      projectPkgNode: this.collectProjectHarDependencyForHap(d, s, t)
    };
  }
  getDependenciesForPkgInfo(e, n, o, t) {
    this._allDependency.clear();
    const s = [],
      d = new Map(),
      r = this.buildSelfDependency(e, n),
      l = {
        moduleName: this._moduleName,
        modulePkgJsonPath: n,
        children: [],
        npm: r,
        module: r
      };
    this.collectHarDependency(l), this.collectFromTree(l, s, d, o);
    return {
      npmDependencies: s,
      moduleDependencyMap: d,
      modulePkgNode: l,
      projectPkgNode: this.collectModuleHarDependencyForHap(d, s, t)
    };
  }
  collectModuleHarDependencyForHap(e, n, o) {
    if (!this._projectModel) return;
    const t = this._isOhpmDependency ? this._projectModel.getOhPackageJson5Path() : this._projectModel.getPackageJsonPath(),
      s = this.buildSelfDependency(this._projectModel, t),
      d = {
        moduleName: this._moduleName,
        modulePkgJsonPath: t,
        children: [],
        npm: s,
        module: s
      };
    this.collectHarDependency(d);
    return this.collectFromTree(d, n, e, (e, n, t) => !o || o(e, n, t)), d;
  }
  collectAllHspDependencies() {
    const {
      moduleDependencyMap: e
    } = this.collectAllDependencies();
    return [...e.entries()].filter(([, e]) => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP);
  }
  collectProjectHarDependencyForHap(e, n, o) {
    if (!this._projectModel) return;
    const t = this._isOhpmDependency ? this._projectModel.getOhPackageJson5Path() : this._projectModel.getPackageJsonPath(),
      s = this.buildSelfDependency(this._projectModel, t),
      d = {
        moduleName: this._moduleName,
        modulePkgJsonPath: t,
        children: [],
        npm: s,
        module: s
      };
    this.collectHarDependency(d);
    return this.collectFromTree(d, n, e, (e, n, t) => !(e.module && !this._model.isHapModule()) && (!o || o(e, n, t))), d;
  }
  collectHarDependency(e) {
    const n = [e];
    for (; n.length > 0;) {
      let e = n.shift();
      if (undefined === e || !fs_1.existsSync(e.modulePkgJsonPath)) continue;
      const o = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(e.modulePkgJsonPath),
        t = this.getFinalOhPkgJsonObj(o);
      if (null === t) continue;
      this.resolveDeps(e, n, dependency_interface_js_1.DependencyEnum.DEPENDENCIES, null == t ? undefined : t.dependencies), this.resolveDeps(e, n, dependency_interface_js_1.DependencyEnum.DYNAMIC_DEPENDENCIES, null == t ? undefined : t.dynamicDependencies), this.resolveDeps(e, n, dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES, null == t ? undefined : t.devDependencies);
      const s = `${e.npm.getDependencyName()}@${e.modulePkgJsonPath}`;
      let d = hvigor_1.hvigorCore.getCache(common_const_js_1.CommonConst.DEP_NODE);
      !d.has(s) && d.set(s, e);
    }
  }
  resolveDeps(e, n, o, t) {
    for (const s in null != t ? t : {}) {
      this._allDependency.add(`${e.modulePkgJsonPath}@${s}`);
      const d = t[s];
      if (Object.prototype.hasOwnProperty.call(t, s)) {
        let t = this.collectDependency(s, d, e.moduleName, e, n, o);
        t && e.children.push(t);
      }
    }
  }
  collectFromTree(e, n, o, t) {
    return this.collectDependenciesFromTree({
      root: e,
      npmDeps: n,
      moduleDepMap: o,
      filter: t
    });
  }
  collectDependenciesFromTree(e) {
    const {
        root: n,
        npmDeps: o,
        moduleDepMap: t,
        filter: s,
        shouldCollect: d
      } = e,
      r = [...n.children],
      l = new Set(),
      i = new Map(o.map(e => [e.getDependencyName(), e.getDependencyRootPath()]));
    for (; r.length > 0;) {
      const e = r.shift();
      if (l.has(e.modulePkgJsonPath)) continue;
      if (s && !s(e, true, false)) continue;
      const n = i.get(e.npm.getDependencyName()),
        c = !d || d(e);
      if (!n && c && (null == o || o.push(e.npm), i.set(e.npm.getDependencyName(), e.npm.getDependencyRootPath())), e.module && e.module.isLocal() && c) {
        const n = t.get(e.module.getModuleName());
        n ? (0, dependency_interface_js_1.compareDependencyEnum)(e.module.getDependencyEnum(), n.getDependencyEnum()) && t.set(e.module.getModuleName(), e.module) : t.set(e.module.getModuleName(), e.module);
      }
      s && !s(e, false, true) || (r.push(...e.children), l.add(e.modulePkgJsonPath));
    }
  }
  collectDependency(e, n, o, t, s, d) {
    var r, l, i;
    const c = this.getPkgFileName(),
      a = (this._isOhpmDependency ? new ohpm_package_resolver_js_1.OhpmPackageResolver() : new npm_package_resolver_js_1.NpmPackageResolver()).resolvePackagePath(e, path_1.dirname(t.modulePkgJsonPath), log),
      _ = t.npm.isModuleDependency();
    if (undefined === a || !fs_1.existsSync(a)) return void (fs_1.existsSync(path_1.resolve(this._model.getProjectDir(), common_const_js_1.CommonConst.OH_MODULES)) && !this.isIndirectDevdependency(null == t ? undefined : t.modulePkgJsonPath, d) && log.warn(`The current module '${this._moduleName}' has dependency which is not installed at its ${c}.`));
    const u = hvigor_1.hvigorCore.getCache(common_const_js_1.CommonConst.DEP_NODE).get(`${e}@${a}`);
    if (u) {
      const e = {
        npm: new dependency_builder_js_1.DependencyBuilder().setDependencyName(u.npm.getDependencyName()).setDependedVersion(n).setDependencyRootPath(u.npm.getDependencyRootPath()).setPkgJsonPath(u.npm.getPackageFilePath()).setPkgJsonObj(u.npm.getPackageJsonObj()).setDependencyType(u.npm.getDependencyType()).setDependencyEnum(d).setModuleJsonObj(u.npm.getModuleJsonObj()).setIsLocal(u.npm.isLocal()).setLastDependencyName(o).setIsModuleDependency(_).buildDefaultNpmDependency(),
        parent: t,
        children: [...u.children],
        moduleName: u.moduleName,
        modulePkgJsonPath: u.modulePkgJsonPath
      };
      return u.module && (e.module = new dependency_builder_js_1.DependencyBuilder().setModule(u.module.getModule()).setDependencyName(u.module.getDependencyName()).setDependedVersion(n).setDependencyRootPath(u.module.getDependencyRootPath()).setPkgJsonPath(u.module.getPackageJsonPath()).setPkgJsonObj(u.module.getPackageJsonObj()).setDependencyType(u.module.getDependencyType()).setDependencyEnum(d).setIsLocal(true).setModuleJsonObj(u.module.getModuleJsonObj()).setLastDependencyName(o).setIsModuleDependency(_).buildDefaultModuleDependency()), e;
    }
    const p = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(a),
      m = this.getFinalOhPkgJsonObj(p),
      h = this.getDependencyType(a, m, t, e);
    switch (h) {
      case dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAP:
        log.warn(`HAP dependency (module "${e}") detected in path "${t.modulePkgJsonPath}".\n          Try the following:\n          1. Remove the dependency on module "${e}".\n          2. Change module "${e}" to a HAR or HSP.`);
      case undefined:
        return;
    }
    const y = path_1.dirname(a);
    this.checkHarStatusIsCompatibleForHap(y, m), this._allDependency.add(`${a}@${e}`);
    const g = null === (r = hvigor_1.hvigorCore.getProject()) || undefined === r ? undefined : r.getAllSubModules();
    if (!g) return;
    const D = null !== (l = this.getModuleJsonObj(new base_package_resolver_js_1.BasePackageResolver("src/main/module.json5", common_const_js_1.CommonConst.OH_MODULES).resolvePackagePath(e, path_1.dirname(t.modulePkgJsonPath), log))) && undefined !== l ? l : this.getModuleJsonObj(new base_package_resolver_js_1.BasePackageResolver("src/main/module.json", common_const_js_1.CommonConst.OH_MODULES).resolvePackagePath(e, path_1.dirname(t.modulePkgJsonPath), log));
    let f,
      j = a,
      P = false;
    const v = g.find(e => e.getNodeDir() === y);
    if (v) {
      j = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.join(v.getNodeDir(), c));
      P = true, f = new dependency_builder_js_1.DependencyBuilder().setModule(v).setDependencyName(e).setDependedVersion(n).setLastDependencyName(t.moduleName).setDependencyRootPath(v.getNodeDir()).setPkgJsonPath(j).setPkgJsonObj(m).setDependencyType(h).setDependencyEnum(d).setIsLocal(true).setModuleJsonObj(D).setIsModuleDependency(_).buildDefaultModuleDependency();
    }
    const E = oh_package_loader_js_1.ohPackageLoader.getOriginPathFromUpdatePath([j])[0],
      M = new dependency_builder_js_1.DependencyBuilder().setDependencyName(e).setDependedVersion(n).setDependencyRootPath(E).setPkgJsonPath(j).setPkgJsonObj(m).setDependencyType(h).setDependencyEnum(d).setModuleJsonObj(D).setIsLocal(P).setIsModuleDependency(_).buildDefaultNpmDependency(),
      N = {
        moduleName: null !== (i = null == f ? undefined : f.getModuleName()) && undefined !== i ? i : "",
        modulePkgJsonPath: a,
        parent: t,
        children: [],
        npm: M,
        module: f
      };
    return h !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR && h !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER && h !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP || s.push(N), N;
  }
  getDependencyType(e, n, o, t) {
    var s, d;
    return null !== (d = null !== (s = this.getHarmonyDependencyType(e, n, o.modulePkgJsonPath, t)) && undefined !== s ? s : this.getSODependencyType(n.name)) && undefined !== d ? d : dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER;
  }
  getPkgFileName() {
    return this._isOhpmDependency ? common_const_js_1.CommonConst.OH_PACKAGE_JSON5 : common_const_js_1.CommonConst.PACKAGE_JSON;
  }
  getModuleJsonObj(e) {
    if (undefined !== e && fs_1.existsSync(e)) return (0, dependency_util_js_1.getJson5Obj)(e);
  }
  getDependencyTypeByProfile(e) {
    const n = path_1.resolve(e, "src", "main", common_const_js_1.CommonConst.MODULE_JSON5),
      o = path_1.resolve(e, "src", "main", common_const_js_1.CommonConst.MODULE_JSON),
      t = path_1.resolve(e, "src", "main", common_const_js_1.CommonConst.CONFIG_JSON);
    return this.getDependencyTypeByProfileType(n) || this.getDependencyTypeByProfileType(o) || this.getDependencyTypeByProfileType(t);
  }
  getDependencyTypeByProfileType(e) {
    var n, o, t, s, d, r, l, i, c, a;
    if (!fs_1.existsSync(e)) return;
    const _ = (0, dependency_util_js_1.getJson5Obj)(e),
      u = (null === (n = _.module) || undefined === n ? undefined : n.type) === module_type_enum_js_1.ModuleType.Shared,
      p = (null === (o = _.module) || undefined === o ? undefined : o.type) === module_type_enum_js_1.ModuleType.Har || (null === (s = null === (t = _.module) || undefined === t ? undefined : t.distro) || undefined === s ? undefined : s.moduleType) === module_type_enum_js_1.ModuleType.Har,
      m = (null === (d = _.module) || undefined === d ? undefined : d.type) === module_type_enum_js_1.ModuleType.Entry || (null === (r = _.module) || undefined === r ? undefined : r.type) === module_type_enum_js_1.ModuleType.Feature || (null === (i = null === (l = _.module) || undefined === l ? undefined : l.distro) || undefined === i ? undefined : i.moduleType) === module_type_enum_js_1.ModuleType.Entry || (null === (a = null === (c = _.module) || undefined === c ? undefined : c.distro) || undefined === a ? undefined : a.moduleType) === module_type_enum_js_1.ModuleType.Feature;
    return u ? dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP : p ? dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR : m ? dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAP : undefined;
  }
  checkHspOrHarStatusByProfile(e) {
    const n = path_1.resolve(e, "src", "main", common_const_js_1.CommonConst.MODULE_JSON5),
      o = path_1.resolve(e, "src", "main", common_const_js_1.CommonConst.MODULE_JSON),
      t = path_1.resolve(e, "src", "main", common_const_js_1.CommonConst.CONFIG_JSON);
    return this.checkHspOrHarStatus(n, false) || this.checkHspOrHarStatus(o, false) || this.checkHspOrHarStatus(t, true);
  }
  checkHspOrHarStatus(e, n) {
    var o, t, s, d;
    if (!fs_1.existsSync(e)) return false;
    const r = (0, dependency_util_js_1.getJson5Obj)(e),
      l = (null === (o = r.module) || undefined === o ? undefined : o.type) === module_type_enum_js_1.ModuleType.Shared;
    return ((null === (t = r.module) || undefined === t ? undefined : t.type) === module_type_enum_js_1.ModuleType.Har || (null === (d = null === (s = r.module) || undefined === s ? undefined : s.distro) || undefined === d ? undefined : d.moduleType) === module_type_enum_js_1.ModuleType.Har || l) && n && log.printErrorExit("MODEL_NOT_CONSISTENT", [this._moduleName], [[solution, e]]), true;
  }
  getHarmonyDependencyType(e, n, o, t) {
    var s, d;
    if (!e || null === n) return;
    const r = path_1.dirname(e),
      l = null !== (d = null === (s = oh_package_loader_js_1.ohPackageLoader.getOriginPathFromUpdatePath([e])) || undefined === s ? undefined : s[0]) && undefined !== d ? d : r,
      i = this.isHarForDiffPackManagement(r, n);
    this._isFaMode ? this.legacyDependencyManagerHelper.checkHspOrHarStatusForFAByProfile(r, n) : this.checkHspOrHarStatusByProfile(r);
    const c = this.getDependencyTypeByProfile(l);
    return i && this._allDependency.has(`${e}@${t}`) && e === o && c === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP && log.printErrorExit("HSP_RELY_HSP", [this._moduleName, r]), i && n.name ? c : undefined;
  }
  isHarForDiffPackManagement(e, n) {
    return this._isOhpmDependency ? fs_1.existsSync(e) : fs_1.existsSync(e) && undefined !== n.ohos;
  }
  checkHarStatusIsCompatibleForHap(e, n) {
    var o;
    if (this._model instanceof core_project_model_impl_js_1.CoreProjectModelImpl) return;
    const t = this._model;
    if (t.isHarModule()) return;
    const s = this._isOhpmDependency ? n.artifactType : null === (o = n.ohos) || undefined === o ? undefined : o.artifactType;
    undefined !== s && s !== har_extend_info_js_1.ArtifactType.ORIGINAL && t instanceof legacy_module_model_impl_js_1.LegacyModuleModelImpl && log.printErrorExit("FA_RELY_OH_NPM", [this._moduleName, e], [[solution]]);
  }
  static isLocalDependencyForSo(e) {
    return !e.includes("oh_modules");
  }
  static isLocalDependency(e, n) {
    return n.has(e.getDependencyRootPath());
  }
  static getModuleNameFromOhPkgJson(e) {
    var n;
    const o = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(e, common_const_js_1.CommonConst.OH_PACKAGE_JSON5));
    if (!fs_1.existsSync(o)) return "";
    const t = (0, dependency_util_js_1.getJson5Obj)(o);
    return null !== (n = null == t ? undefined : t.name) && undefined !== n ? n : "";
  }
  static getRemoteDependencyRouterMapObjList(e) {
    var n, o;
    const t = path_1.resolve(e.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN),
      s = this.getModuleObjByRemoteDependency(e),
      d = null !== (n = (0, common_util_js_1.parsingProfileName)(null == s ? undefined : s.module.routerMap)) && undefined !== n ? n : undefined;
    if (!d) return;
    const r = path_1.resolve(t, build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR, `${d}.json`),
      l = (0, dependency_util_js_1.getJson5Obj)(r, true);
    return l.routerMap.forEach(n => {
      Object.assign(n, {
        moduleNodeDir: e.getDependencyRootPath()
      });
    }), null !== (o = l.routerMap) && undefined !== o ? o : undefined;
  }
  static getLocalDependencyRouterMapObjList(e, n, o = common_const_js_1.DefaultTargetConst.DEFAULT_TARGET) {
    var t;
    if (!e) return;
    const s = this.getModuleModelByDependency(e, n);
    if (!s) return;
    const d = this.getDependencyModuleRouterMapJsonPath(s, o);
    if (!d || !fs_1.existsSync(d)) return;
    const r = (0, dependency_util_js_1.getJson5Obj)(d, true);
    return r.routerMap.forEach(e => {
      Object.assign(e, {
        moduleNodeDir: s.getModule().getNodeDir()
      });
    }), null !== (t = r.routerMap) && undefined !== t ? t : undefined;
  }
  static getDependencyModuleRouterMapJsonPath(e, n = common_const_js_1.DefaultTargetConst.DEFAULT_TARGET) {
    var o, t;
    if (!e) return;
    const s = this.getRouterMapJsonFileName(e);
    if (!s) return;
    const d = [],
      r = e.getModule().getNodeDir(),
      l = e.getProfileOpt();
    if (null === (o = null == l ? undefined : l.targets) || undefined === o ? undefined : o.length) {
      const e = l.targets.find(e => e.name === n);
      d.push(...file_util_js_1.FileUtil.convertToAbsolutePaths((null === (t = null == e ? undefined : e.resource) || undefined === t ? undefined : t.directories) || [], r));
    }
    for (const e of d) {
      const n = path_1.resolve(e, "base", "profile", `${s}.json`);
      if (fs_1.existsSync(n)) return n;
    }
    return path_1.resolve(e.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR, `${s}.json`);
  }
  static getRouterMapJsonFileName(e) {
    var n;
    if (!e) return;
    const o = path_1.resolve(e.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, common_const_js_1.CommonConst.MODULE_JSON5),
      t = res_model_loader_js_1.resModelLoader.getModuleJson(o),
      s = null === (n = null == t ? undefined : t.module) || undefined === n ? undefined : n.routerMap;
    return s ? (0, common_util_js_1.parsingProfileName)(s) : undefined;
  }
  static getRemoteDependencyShareConfigJsonPath(e, n) {
    const o = (0, dependency_util_js_1.getRemoteDepHarTargetSources)(e);
    for (const e of o) {
      const o = path_1.resolve(e, common_const_js_1.CommonConst.BASE_PROFILE, `${n}.json`);
      if (fse.existsSync(o)) return o;
    }
  }
  static getRemoteDependencyShareConfigObjList(e) {
    var n, o;
    const t = this.getModuleObjByRemoteDependency(e);
    if (!t) return;
    const s = null !== (n = (0, common_util_js_1.parsingProfileName)(null == t ? undefined : t.module.crossAppSharedConfig)) && undefined !== n ? n : undefined;
    if (!s) return;
    const d = this.getRemoteDependencyShareConfigJsonPath(e, s);
    if (!d || !fs_1.existsSync(d)) return;
    const r = (0, dependency_util_js_1.getJson5Obj)(d);
    return Array.isArray(null == r ? undefined : r.crossAppSharedConfig) && r.crossAppSharedConfig.forEach(n => {
      Object.assign(n, {
        moduleNodeDir: e.getDependencyRootPath()
      });
    }), null !== (o = r.crossAppSharedConfig) && undefined !== o ? o : undefined;
  }
  static getLocalDependencyShareConfigObjList(e, n, o = common_const_js_1.DefaultTargetConst.DEFAULT_TARGET) {
    var t;
    if (!e) return;
    const s = this.getModuleModelByDependency(e, n);
    if (!s) return;
    const d = this.getDependencyModuleShareConfigJsonPath(s, o);
    if (!d || !fs_1.existsSync(d)) return;
    const r = (0, dependency_util_js_1.getJson5Obj)(d);
    return Array.isArray(null == r ? undefined : r.crossAppSharedConfig) && r.crossAppSharedConfig.forEach(e => {
      Object.assign(e, {
        moduleNodeDir: s.getModule().getNodeDir()
      });
    }), null !== (t = r.crossAppSharedConfig) && undefined !== t ? t : undefined;
  }
  static getDependencyModuleShareConfigJsonPath(e, n = common_const_js_1.DefaultTargetConst.DEFAULT_TARGET) {
    var o, t;
    if (!e) return;
    const s = this.getShareConfigJsonFileName(e);
    if (!s) return;
    const d = [],
      r = e.getModule().getNodeDir(),
      l = e.getProfileOpt();
    if (null === (o = null == l ? undefined : l.targets) || undefined === o ? undefined : o.length) {
      const e = l.targets.find(e => e.name === n);
      d.push(...file_util_js_1.FileUtil.convertToAbsolutePaths((null === (t = null == e ? undefined : e.resource) || undefined === t ? undefined : t.directories) || [], r));
    }
    for (const e of d) {
      const n = path_1.resolve(e, "base", "profile", `${s}.json`);
      if (fs_1.existsSync(n)) return n;
    }
    return path_1.resolve(e.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR, `${s}.json`);
  }
  static getShareConfigJsonFileName(e) {
    if (!e) return;
    const n = path_1.resolve(e.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, common_const_js_1.CommonConst.MODULE_JSON5),
      o = res_model_loader_js_1.resModelLoader.getModuleJson(n).module.crossAppSharedConfig;
    return (0, common_util_js_1.parsingProfileName)(o);
  }
  static getModuleModelByDependency(e, n) {
    if (e.isLocal()) return n.getAllModules().find(n => n.getProjectDir() === e.getDependencyRootPath());
  }
  static getModuleObjByRemoteDependency(e) {
    const n = path_1.resolve(e.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN),
      o = path_1.resolve(n, common_const_js_1.CommonConst.MODULE_JSON),
      t = path_1.resolve(n, common_const_js_1.CommonConst.MODULE_JSON5),
      s = fs_1.existsSync(o) ? o : fs_1.existsSync(t) ? t : "";
    return (0, dependency_util_js_1.getJson5Obj)(s);
  }
  getSODependencyType(e) {
    return (null == e ? undefined : e.includes(".so")) ? dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_SO : undefined;
  }
  isIndirectDevdependency(e, n) {
    return !!e && (0, copy_resources_util_js_1.toUnixPath)(e).includes("/oh_modules/") && n === dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES;
  }
  getFinalOhPkgJsonObj(e) {
    let n = (0, dependency_util_js_1.getJson5Obj)(e);
    return n && n.name && oh_modules_lock_loader_js_1.ohModulesLockLoader.isNeedOverrideWithLockJson(n.name) && (n = oh_modules_lock_loader_js_1.ohModulesLockLoader.getOverrideDependencyPkgObj(n.name, n)), n;
  }
}
export function getEntryMainFilePath(e, n) {
  const o = path_1.resolve(e, (0, hvigor_arkts_compose_1.findNodeEntryFiles)(n)[0]);
  if (fs_1.existsSync(o)) return o;
  {
    const o = getEntryPath(n.main, n.types),
      t = path_1.resolve(e, o);
    return fs_1.existsSync(t) ? t : "";
  }
}
export function getEntryPath(e, n) {
  return e && "" !== e ? e : n && "" !== n ? n : build_directory_const_js_1.BuildArtifactConst.INDEX_ETS;
}
