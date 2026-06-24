/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/task-service.js
 */
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as dependency_interface_js_1 from "../../project/dependency/core/dependency-interface";
import * as copy_resources_util_js_1 from "../../utils/copy-resources-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as remote_hsp_utils_js_1 from "../../utils/remote-hsp-utils";
var __decorate = this && this.__decorate || function (e, n, t, c) {
  var p,
    s = arguments.length,
    d = s < 3 ? n : null === c ? c = Object.getOwnPropertyDescriptor(n, t) : c;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) d = Reflect.decorate(e, n, t, c);else for (var o = e.length - 1; o >= 0; o--) (p = e[o]) && (d = (s < 3 ? p(d) : s > 3 ? p(n, t, d) : p(n, t)) || d);
  return s > 3 && d && Object.defineProperty(n, t, d), d;
};
var DependencyCacheKey;
function cacheable(e) {
  return function (n, t, c) {
    const p = c.value;
    "function" == typeof p && (c.value = function (...n) {
      const t = this.getDependencyCache();
      if (t.has(e)) return t.get(e);
      const c = p.apply(this, n);
      return t.set(e, c), c;
    });
  };
}
!function (e) {
  e[e.AllHspDependencies = 0] = "AllHspDependencies", e[e.RootByteCodeHarDependencies = 1] = "RootByteCodeHarDependencies", e[e.RootOtherDependencies = 2] = "RootOtherDependencies", e[e.RootHspDependencies = 3] = "RootHspDependencies", e[e.AffectsCompilationDependencies = 4] = "AffectsCompilationDependencies", e[e.AllByteCodeHarDependencies = 5] = "AllByteCodeHarDependencies", e[e.AllModuleDependencies = 6] = "AllModuleDependencies";
}(DependencyCacheKey || (DependencyCacheKey = {}));
export class TaskService {
  constructor(e, n, t, c) {
    this.log = ohos_logger_js_1.OhosLogger.getLogger("TaskService"), this._dependencyCache = new Map(), this.hvigorNode = e, this._projectModel = n, this._dependencyManager = t, this._isFaMode = c;
  }
  getDependencyCache() {
    return this._dependencyCache;
  }
  initDependencyInfo() {
    this.log.debug("Start to initialize dependency information."), this._moduleDependencyInfo = this._dependencyManager.createModelDependencyInfo();
  }
  isFaMode() {
    return this._isFaMode;
  }
  getNode() {
    return this.hvigorNode;
  }
  getProjectModel() {
    return this._projectModel;
  }
  getDependencyMainPaths() {
    return this.getDependencyInfo().getNpmDependencies().map(e => e.getDependencyMainFilePath());
  }
  getDependencyRootPaths() {
    return this.getDependencyInfo().getNpmDependencies().map(e => e.getDependencyRootPath());
  }
  getAllModuleOrProjectDependencies() {
    return this._dependencyManager.collectModelOrProjectAllDependencyInfo();
  }
  getAllOtherDependencies() {
    return this._dependencyManager.collectModelOrProjectAllDependencyInfo().filter(e => !e.isLocal());
  }
  getDependencyName2RootPath() {
    const e = new Map();
    for (const n of this._dependencyManager.collectAllDependenciesForPkgInfo().npmDependencies) {
      const t = n.getDependencyName();
      if (e.has(t)) return false;
      e.set(t, n.getDependencyRootPath());
    }
    return e;
  }
  getDependencyName2DepInfo() {
    const e = new Map(),
      n = this.getDependencyInfo().getSelfAsDependency();
    n.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR && e.set(n.getDependencyName(), {
      dependencyType: n.getDependencyType(),
      isByteCodeHar: n.isByteCodeHarDependency(),
      pkgRootPath: n.getDependencyRootPath(),
      pkgName: n.getPackageName(),
      pkgVersion: n.getDependencyVersion()
    });
    for (const n of this._dependencyManager.collectAllDependenciesForPkgInfo().npmDependencies) {
      const t = n.getDependencyName();
      if (e.has(t)) return new Map();
      e.set(t, {
        dependencyType: n.getDependencyType(),
        isByteCodeHar: n.isByteCodeHarDependency(),
        pkgRootPath: n.getDependencyRootPath(),
        pkgName: n.getPackageName(),
        pkgVersion: n.getDependencyVersion()
      });
    }
    return e;
  }
  getDependencyName2OhExportsMap() {
    const e = new Map();
    for (const n of this._dependencyManager.collectAllDependenciesForPkgInfo().npmDependencies) {
      const t = this.collectDependencyOhExports(n);
      t && e.set(n.getDependencyName(), {
        pkgJsonPath: n.getPackageFilePath(),
        [common_const_js_1.CommonConst.OH_EXPORTS]: t
      });
    }
    return e;
  }
  collectDependencyOhExports(e) {
    const n = e.getOhExportsFilePaths();
    if (!n) return;
    const t = new Set();
    return n.forEach(e => {
      t.add((0, copy_resources_util_js_1.toUnixPath)(e));
    }), t;
  }
  getHarDependencies() {
    return this.getDependencyInfo().getNpmDependencies().filter(e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR);
  }
  getRemoteHarDependencies() {
    return this.getHarDependencies().filter(e => !e.isLocal());
  }
  getHspDependencies() {
    return this.getDependencyInfo().getNpmDependencies().filter(e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP);
  }
  getAllDependencyType() {
    const e = new Map(),
      n = this.getAllDependencies();
    for (const t of n) e.set(t.getDependencyName(), t.getDependencyType());
    return e;
  }
  getByteCodeHar2Hsp() {
    const e = new Map(),
      n = this.getDependencyInfo().getNpmDependencies();
    return n.forEach(t => {
      if (t.isByteCodeHarDependency()) {
        const c = t.getDependencyName();
        e.set(c, this.getHspRecursion(c, n));
      }
    }), e;
  }
  getHspRecursion(e, n, t = new Map()) {
    if (t.has(e)) return t.get(e) || [];
    const c = [];
    t.set(e, c);
    const p = n.find(n => n.getDependencyName() === e);
    if (!(null == p ? undefined : p.getDependencies())) return this.log.debug(`No dependency is found: ${e}`), [];
    return Object.keys(p.getDependencies()).forEach(e => {
      const p = n.find(n => n.getDependencyName() === e);
      (null == p ? undefined : p.getDependencyType()) === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP ? c.push(e) : c.push(...this.getHspRecursion(e, n, t));
    }), c;
  }
  getHspOrByteCodeHarDependencies() {
    return this.getDependencyInfo().getNpmDependencies().filter(e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP || e.isByteCodeHarDependency());
  }
  getSODependencies() {
    return this.getDependencyInfo().getAllDependencies().filter(e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_SO);
  }
  getOtherDependencies() {
    return this.getDependencyInfo().getAllDependencies().filter(e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER);
  }
  getAllDependencies() {
    return this.getDependencyInfo().getAllDependencies();
  }
  excludeChildrenWhenInDevDependencies(e) {
    return e ? e.children.filter(e => e.npm.getDependencyEnum() !== dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES) : [];
  }
  getDirectPkgNodes() {
    const e = this.getDependencyInfo(),
      n = e.getModulePkgNode(),
      t = this.excludeChildrenWhenInDevDependencies(e.getModulePkgNode());
    let c = this.excludeChildrenWhenInDevDependencies(e.getProjectPkgNode());
    if (!c.length) return t;
    n.npm.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAP || (c = c.filter(e => !e.npm.isLocal()));
    const p = new Set(t.map(e => e.npm.getDependencyName()));
    return c = c.filter(e => !p.has(e.npm.getDependencyName())), t.concat(c);
  }
  getDirectDependencies() {
    return this.getDirectPkgNodes().map(e => e.npm);
  }
  getAllDirectPkgNodes() {
    return this.getDependencyInfo().getDirectPkgNodes();
  }
  getAllDirectDependencies() {
    return this.getAllDirectPkgNodes().map(e => e.npm);
  }
  collectFromTree(e, n) {
    const t = this.getDependencyInfo().getModulePkgNode(),
      c = t.npm.getDependencyRootPath(),
      p = {
        ...t,
        children: this.getAllDirectPkgNodes()
      },
      s = [],
      d = new Map();
    return this._dependencyManager.collectDependenciesFromTree({
      root: p,
      npmDeps: s,
      moduleDepMap: d,
      filter: (n, t, p) => {
        const s = n.npm;
        return s.getDependencyRootPath() === c ? (this.log.debug(`Exclude current module dependency: ${s.getPackageName()}`), false) : !e(n, t, p);
      },
      shouldCollect: n
    }), {
      npmDependencies: s,
      moduleDepMap: d
    };
  }
  getAllHspDependencies() {
    return this.collectFromTree(e => e.npm.isOtherDependency(), e => e.npm.isHspDependency()).npmDependencies;
  }
  getRootByteCodeHarDependencies() {
    return this.getAffectsCompilationDependencies().filter(e => e.isByteCodeHarDependency());
  }
  getRootOtherDependencies() {
    return this.getAffectsCompilationDependencies().filter(e => e.isOtherDependency());
  }
  getRootHspDependencies() {
    return this.collectFromTree((e, n, t) => {
      const c = e.npm;
      return !!c.isOtherDependency() || t && c.isHspDependency();
    }, e => e.npm.isHspDependency()).npmDependencies;
  }
  getAffectsCompilationDependencies() {
    return this.collectFromTree((e, n, t) => t && (e.npm.isHspDependency() || e.npm.isByteCodeHarDependency() || e.npm.isOtherDependency()), () => true).npmDependencies;
  }
  getAllByteCodeHarDependencies() {
    return this.collectFromTree(e => e.npm.isOtherDependency() || e.npm.isHspDependency(), e => e.npm.isByteCodeHarDependency()).npmDependencies;
  }
  getAllModuleDependencies() {
    return this.collectFromTree((e, n, t) => e.npm.isOtherDependency() || t && e.npm.isHspDependency(), e => e.npm.isLocal()).npmDependencies;
  }
  getHspDependencyPaths() {
    return this.getHspDependencies().filter(e => e.getDependencyEnum() !== dependency_interface_js_1.DependencyEnum.DYNAMIC_DEPENDENCIES).map(e => e.getDependencyRootPath());
  }
  getModuleDependencies() {
    return [...this.getDependencyInfo().getModuleDependencyMap().values()];
  }
  getModuleDependenciesPaths() {
    return this.getModuleDependencies().map(e => e.getDependencyRootPath());
  }
  getHarModuleDependencies() {
    return [...this.getDependencyInfo().getModuleDependencyMap().entries()].filter(([, e]) => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR);
  }
  getHarModuleDependenciesWithRootPath() {
    const e = new Map();
    return this.getDependencyInfo().getModuleDependenciesByType(dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR).forEach(([n, t]) => {
      e.set(t.getDependencyRootPath(), [n, t]);
    }), e;
  }
  getHarModuleDependencyNames() {
    return this.getHarModuleDependencies().map(([e]) => e);
  }
  getHarModuleDependencyPaths() {
    return this.getHarModuleDependencies().map(([, e]) => e.getDependencyRootPath());
  }
  getHspModuleDependencies() {
    return [...this.getDependencyInfo().getModuleDependencyMap().entries()].filter(([, e]) => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP);
  }
  getHspModuleDependencyNames() {
    return this.getHspModuleDependencies().map(([e]) => e);
  }
  getHspModuleDependencyPaths() {
    return this.getHspModuleDependencies().map(([, e]) => e.getDependencyRootPath());
  }
  getHspModuleDependencyPathsWithOutDynamic() {
    return this.getHspModuleDependencies().filter(([, e]) => e.getDependencyEnum() !== dependency_interface_js_1.DependencyEnum.DYNAMIC_DEPENDENCIES).map(([, e]) => e.getDependencyRootPath());
  }
  hasHspDependencies() {
    return this.getDependencyInfo().hasHspDependency();
  }
  getDependencyInfo() {
    return !this._moduleDependencyInfo && this.initDependencyInfo(), this._moduleDependencyInfo;
  }
  getOhpmDependencyInfo() {
    const e = {};
    return this.getDependencyInfo().getNpmDependencies().forEach(n => {
      e[n.getDependencyName()] = {
        name: n.getPackageName(),
        version: n.getDependencyVersion(),
        dependencies: n.getDependencies(),
        packagePath: n.getDependencyRootPath()
      };
    }), e;
  }
  getOhpmRemoteHspAllInfo(e, n, t) {
    const c = {},
      p = this._dependencyManager.collectAllDependencies().npmDependencies,
      s = (0, remote_hsp_utils_js_1.getRemoteHspPathMap)(e, p);
    return p.forEach(e => {
      this.setDependencyInfo(e, s, t, n, c);
    }), c;
  }
  setDependencyInfo(e, n, t, c, p) {
    var s;
    if (e.isLocal() || e.getDependencyType() !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP) return;
    const d = this.getRemoteHspDirName(e.getDependencyRootPath()),
      o = n.get(e.getPackageName());
    if (!d || !o) return void this.log.debug(`${e.getDependencyRootPath()} does not exist oh_modules.`);
    let r;
    const i = e.getPackageJsonObj(),
      a = path_1.resolve(e.getDependencyRootPath(), "libs");
    if (t) {
      const n = path_1.resolve(this._projectModel.getCacheRemoteHspPath(null != c ? c : "default"), d, o ? o.replace(build_directory_const_js_1.BuildArtifactExtension.DOT_HSP, `-signed${build_directory_const_js_1.BuildArtifactExtension.DOT_HSP}`) : `${e.getPackageName()}-signed.hsp`);
      p[e.getDependencyName()] = {
        name: e.getPackageName(),
        version: e.getDependencyVersion(),
        dependencies: e.getDependencies(),
        packagePath: e.getDependencyRootPath(),
        signedRemoteHspPath: n,
        soPath: a
      };
    } else r = (null === (s = null == i ? undefined : i.metadata) || undefined === s ? undefined : s.integratedHsp) ? path_1.resolve(this._projectModel.getCacheIntegratedHspPath(null != c ? c : "default"), d, o || `${e.getPackageName()}.hsp`) : path_1.resolve(this.hvigorNode.getNodeDir(), "oh_modules", ".hsp", d, o || `${e.getPackageName()}.hsp`), p[e.getDependencyName()] = {
      name: e.getPackageName(),
      version: e.getDependencyVersion(),
      dependencies: e.getDependencies(),
      packagePath: e.getDependencyRootPath(),
      remoteHspPath: r,
      soPath: a
    };
  }
  getOhpmRemoteHspDependencies(e, n, t) {
    const c = this.getOhpmRemoteHspAllInfo(e, n, t),
      p = {};
    return Object.entries(c).forEach(([e, n]) => {
      p[e] = t ? {
        name: n.name,
        version: n.version,
        dependencies: n.dependencies,
        packagePath: n.packagePath,
        signedRemoteHspPath: n.signedRemoteHspPath
      } : {
        name: n.name,
        version: n.version,
        dependencies: n.dependencies,
        packagePath: n.packagePath,
        remoteHspPath: n.remoteHspPath
      };
    }), p;
  }
  getRemoteHspDirName(e) {
    const n = (0, copy_resources_util_js_1.toUnixPath)(e).split("/"),
      t = n.lastIndexOf("oh_modules");
    if (t > 0) return n[t - 1];
  }
}
__decorate([cacheable(DependencyCacheKey.AllHspDependencies)], TaskService.prototype, "getAllHspDependencies", null), __decorate([cacheable(DependencyCacheKey.RootByteCodeHarDependencies)], TaskService.prototype, "getRootByteCodeHarDependencies", null), __decorate([cacheable(DependencyCacheKey.RootOtherDependencies)], TaskService.prototype, "getRootOtherDependencies", null), __decorate([cacheable(DependencyCacheKey.RootHspDependencies)], TaskService.prototype, "getRootHspDependencies", null), __decorate([cacheable(DependencyCacheKey.AffectsCompilationDependencies)], TaskService.prototype, "getAffectsCompilationDependencies", null), __decorate([cacheable(DependencyCacheKey.AllByteCodeHarDependencies)], TaskService.prototype, "getAllByteCodeHarDependencies", null), __decorate([cacheable(DependencyCacheKey.AllModuleDependencies)], TaskService.prototype, "getAllModuleDependencies", null);
