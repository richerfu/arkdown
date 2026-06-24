/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/module-dependency-info.js
 */
import * as dependency_interface_js_1 from "./core/dependency-interface";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class ModuleDependencyInfo {
  constructor(e, n, t, d, c) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger("module-dependency-info"), this._self = e, this._modulePkgNode = n, this._projectPkgNode = t, this._moduleDependencyMap = d, this._moduleDependencyCategory = {}, d.forEach((e, n) => {
      const t = e.getDependencyType();
      this._moduleDependencyCategory[t] ? this._moduleDependencyCategory[t].push([n, e]) : this._moduleDependencyCategory[t] = [[n, e]];
    });
    const s = {};
    c.forEach(e => {
      s[e.getDependencyType()] ? s[e.getDependencyType()].push(e) : s[e.getDependencyType()] = [e];
    }), this._npmDependenciesObj = s, this._npmDependencies = [...(s[dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR] || []), ...(s[dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP] || [])], this._allDependencies = c;
    const p = s[dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP];
    this._hasHspDependency = (null == p ? undefined : p.length) > 0;
  }
  keepDependencyNameUnique(e) {
    const n = [],
      t = new Set();
    return e.forEach(e => {
      const d = e.npm.getDependencyName();
      t.has(d) ? this._log.debug(`Dependency key: ${d} already exists, current dependency listed in ${e.npm.getDependencyEnum()} will be ignored.`) : (t.add(d), n.push(e));
    }), n;
  }
  getMergedPkgNodes(e, n) {
    return (null == n ? undefined : n.length) ? [...e.filter(e => e.npm.getDependencyEnum() !== dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES), ...n.filter(e => e.npm.getDependencyEnum() !== dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES), ...e.filter(e => e.npm.getDependencyEnum() === dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES), ...n.filter(e => e.npm.getDependencyEnum() === dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES)] : e;
  }
  backFillPkgNodes(e, n) {
    var t;
    const d = e.npm;
    if (!d.isByteCodeHarDependency()) return;
    const c = d.getPackageJsonObj(),
      s = (null === (t = null == c ? undefined : c.metadata) || undefined === t ? undefined : t.globalDependencies) || [];
    if (!s.length) return;
    const p = [];
    s.forEach(e => {
      const t = n.find(n => n.npm.getDependencyName() === e);
      t ? p.push(t) : this._log.debug(`Global dependency[${e}] used in byteCodeHar[${d.getPackageName()}] does not configure in current module or project.`);
    }), e.children.push(...p.map(n => ({
      ...n,
      parent: e
    })));
  }
  buildDirectPkgNodes() {
    var e;
    const n = this._modulePkgNode.children,
      t = null === (e = this._projectPkgNode) || undefined === e ? undefined : e.children.filter(e => e.npm.getDependencyRootPath() !== this._self.getDependencyRootPath()),
      d = this.keepDependencyNameUnique(this.getMergedPkgNodes(n, t)),
      c = d.map(e => ({
        ...e
      })),
      s = [...c],
      p = new Map();
    for (; s.length;) {
      const e = s.shift(),
        n = e.npm.getDependencyRootPath();
      p.has(n) ? e.children = p.get(n).children : (e.children = this.keepDependencyNameUnique(e.children).map(n => ({
        ...n,
        parent: e
      })), this.backFillPkgNodes(e, d), s.push(...e.children), p.set(n, e));
    }
    return c;
  }
  getDirectPkgNodes() {
    return this._directPkgNodes || (this._directPkgNodes = this.buildDirectPkgNodes()), this._directPkgNodes;
  }
  getSelfAsDependency() {
    return this._self;
  }
  getModulePkgNode() {
    return this._modulePkgNode;
  }
  getProjectPkgNode() {
    return this._projectPkgNode;
  }
  getModuleDependencyMap() {
    return this._moduleDependencyMap;
  }
  getModuleDependenciesByType(e) {
    const n = this._moduleDependencyCategory[e];
    return null != n ? n : [];
  }
  getNpmDependencies() {
    return this._npmDependencies;
  }
  getAllDependencies() {
    return this._allDependencies;
  }
  hasHspDependency() {
    return this._hasHspDependency;
  }
  getHspDependencies() {
    const e = this._npmDependenciesObj[dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP];
    return null != e ? e : [];
  }
  getHarDependencies() {
    const e = this._npmDependenciesObj[dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR];
    return null != e ? e : [];
  }
}
