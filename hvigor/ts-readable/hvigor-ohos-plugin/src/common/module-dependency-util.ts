/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/module-dependency-util.js
 */
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
class ModuleDependencyUtil {
  constructor() {
    this.pureHar = new Set();
  }
  getPureHar(e, n, d, t) {
    return 0 === this.pureHar.size && this.init(e, n, d, t), this.pureHar;
  }
  init(e, n, d, t) {
    if (n.getModuleType() !== module_type_enum_js_1.ModuleType.Entry) return;
    e.getDirectDependencies().filter(e => e.isLocal() && e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR).forEach(e => {
      this.collectPureHar(d, e, t);
    });
  }
  collectPureHar(e, n, d) {
    var t, r, c;
    const o = e.getModuleModelByName(null !== (c = null === (r = null === (t = n.getModuleJsonObj()) || undefined === t ? undefined : t.module) || undefined === r ? undefined : r.name) && undefined !== c ? c : n.getPackageName());
    if (o) {
      const n = new dependency_manager_js_1.DependencyManager(d, o, e).collectAllDependencies().npmDependencies,
        t = n.some(e => e.isLocal() && e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP);
      t || (this.pureHar.add(o.getName()), n.forEach(e => {
        var n, d, t;
        this.pureHar.add(e.isLocal() && null !== (t = null === (d = null === (n = e.getModuleJsonObj()) || undefined === n ? undefined : n.module) || undefined === d ? undefined : d.name) && undefined !== t ? t : e.getPackageName());
      }));
    }
  }
}
export const moduleDependencyUtil = new ModuleDependencyUtil();
