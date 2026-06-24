/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/source-roots-utils.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as hvigor_extra_config_info_js_1 from "../common/hvigor-extra-config-info";
import * as module_path_info_iml_js_1 from "../common/iml/module-path-info-iml";
import * as ohos_plugin_id_js_1 from "../plugin/common/ohos-plugin-id";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as module_task_service_js_1 from "../tasks/service/module-task-service";
import * as npm_package_resolver_js_1 from "./resolver/npm-package-resolver";
import * as ohpm_package_resolver_js_1 from "./resolver/ohpm-package-resolver";
import * as target_utils_js_1 from "./target-utils";
const isHarOrHsp = e => {
    const t = e.getDependencyType();
    return t === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP || t === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR;
  },
  getPkgRoot2Module = e => {
    const t = new Map();
    return e.forEach(e => {
      isHarOrHsp(e) && t.set(e.getDependencyRootPath(), e.getModule());
    }), t;
  },
  getPkgRoot2Dependency = e => {
    const t = new Map();
    return e.forEach(e => {
      isHarOrHsp(e) && t.set(e.getDependencyRootPath(), e);
    }), t;
  },
  findTargetService = e => {
    for (const t of e.tryTargetNames) {
      const o = e.targetServices.find(e => e.getTargetData().getTargetName() === t);
      if (o && (0, module_task_service_js_1.checkHasTargetApplyProduct)(e.moduleModel, t, e.productName)) return o;
    }
  },
  getTargetSourceRoots = ({
    depPkgRoot: e,
    productName: t,
    dependency: o,
    projectModel: r,
    superTargetName: n,
    pkgRoot2Module: a
  }) => {
    var s, d;
    const g = a.get(e);
    if (!g) return;
    const c = g.getName(),
      i = r.getModuleModelByName(c),
      p = o.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP ? g.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN) : g.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAR_PLUGIN);
    if (!p || !i) return;
    const u = target_utils_js_1.TargetUtils.pushTryTargets({
        projectModel: r,
        moduleName: c,
        superTargetName: n
      }),
      l = p.getNeedExecTargetServiceList(),
      _ = findTargetService({
        tryTargetNames: u,
        targetServices: l,
        moduleModel: i,
        productName: t
      });
    if (!_) return;
    const m = (0, hvigor_extra_config_info_js_1.getBuildRoot)(),
      f = _.getTargetData().getTargetName(),
      h = new module_path_info_iml_js_1.ModulePathInfoIml(i, f, t, m),
      P = h.getModulePath(),
      v = null === (d = null === (s = _.getTargetData().getTargetOpt()) || undefined === s ? undefined : s.source) || undefined === d ? undefined : d.sourceRoots;
    return {
      targetName: f,
      originalSourceRoots: v,
      sourceRoots: [...(v || []), path_1.relative(P, h.getModuleSrcMainPath()), path_1.relative(P, path_1.resolve(h.getGenerateBuildProfileDir(), f))]
    };
  },
  getPkgName2PkgBriefInfo = ({
    entryRoot: e,
    productName: t,
    entryTargetName: o,
    projectModel: r,
    entryDependencies: n,
    pkgRoot2Module: a,
    pkgRoot2Dependency: s,
    packageResolver: d
  }) => {
    const g = {},
      c = new Set(),
      i = [{
        basePath: e,
        targetName: o,
        dependencies: n
      }];
    for (; i.length;) {
      const {
        basePath: e,
        dependencies: o,
        targetName: n
      } = i.shift();
      Object.keys(o).forEach(o => {
        var p;
        const u = d.resolvePackagePath(o, e);
        if (!u) return;
        const l = path_1.dirname(u);
        if (c.has(l)) return;
        c.add(l);
        const _ = s.get(l);
        if (!_) return;
        const m = _.getPackageJsonObj(),
          f = m.name,
          h = {
            ...m.dependencies,
            ...m.dynamicDependencies
          };
        if (!_.isLocal()) return i.push({
          targetName: "",
          dependencies: h,
          basePath: l
        }), void (g[f] = {
          pkgRoot: l,
          pkgName: f,
          sourceRoots: null === (p = null == m ? undefined : m.metadata) || undefined === p ? undefined : p.sourceRoots
        });
        const P = getTargetSourceRoots({
          depPkgRoot: l,
          productName: t,
          dependency: _,
          projectModel: r,
          superTargetName: n,
          pkgRoot2Module: a
        });
        P && (i.push({
          targetName: P.targetName,
          dependencies: h,
          basePath: l
        }), g[f] = {
          pkgRoot: l,
          pkgName: f,
          sourceRoots: P.sourceRoots,
          originalSourceRoots: P.originalSourceRoots
        });
      });
    }
    return g;
  },
  getDependenciesPkgBriefInfo = e => {
    const t = e.getModuleService(),
      o = t.getModuleModel(),
      r = t.getProjectModel(),
      n = r.isOhpmProject(),
      a = n ? o.getOhPackageJson5Path() : o.getPackageJsonPath(),
      s = hvigor_1.Json5Reader.getJson5Obj(a),
      d = n ? r.getOhPackageJson5Path() : r.getPackageJsonPath(),
      g = hvigor_1.Json5Reader.getJson5Obj(d),
      c = {
        ...(null == g ? undefined : g.dependencies),
        ...(null == g ? undefined : g.dynamicDependencies),
        ...(null == s ? undefined : s.dependencies),
        ...(null == s ? undefined : s.dynamicDependencies)
      };
    if (!Object.keys(c).length) return {};
    const i = e.getTargetData(),
      p = i.getTargetName(),
      u = n ? new ohpm_package_resolver_js_1.OhpmPackageResolver() : new npm_package_resolver_js_1.NpmPackageResolver(),
      l = new dependency_manager_js_1.DependencyManager(r.isFaMode(), o, r),
      {
        npmDependencies: _,
        moduleDependencyMap: m
      } = l.collectAllDependencies(),
      f = getPkgRoot2Dependency(_),
      h = getPkgRoot2Module(m);
    return getPkgName2PkgBriefInfo({
      projectModel: r,
      pkgRoot2Module: h,
      packageResolver: u,
      pkgRoot2Dependency: f,
      entryTargetName: p,
      productName: i.getProduct().name,
      entryRoot: o.getModule().getNodeDir(),
      entryDependencies: c
    });
  };
export { getDependenciesPkgBriefInfo };
