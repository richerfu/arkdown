/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/har-target-util.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as task_util_js_1 from "@ohos/hvigor/src/base/util/task-util";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_plugin_id_js_1 from "../plugin/common/ohos-plugin-id";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as file_util_js_1 from "./file-util";
import * as json_util_js_1 from "./json-util";
import * as ohos_logger_js_1 from "./log/ohos-logger";
import * as target_utils_js_1 from "./target-utils";
const DEP_TARGET_CACHE_NAME = "depTargetsCache";
export class HarTargetUtil {
  static getHarTargetName(e, t) {
    const o = e.getModuleService().getProjectModel().getModuleSpecificTargets().get(t);
    if ((null == o ? undefined : o.length) && o[0] !== common_const_js_1.DefaultTargetConst.ALL_TARGET) return o[0];
    const r = e.getTargetData().getTargetName();
    if (r === common_const_js_1.CommonConst.OHOS_TEST) return common_const_js_1.DefaultTargetConst.DEFAULT_TARGET;
    const i = e.getModuleService().getProjectModel().getModuleModelByName(t);
    if (!i) return common_const_js_1.DefaultTargetConst.DEFAULT_TARGET;
    const s = i.getTargetOptions().map(e => e.name),
      a = (0, task_util_js_1.getAlignTarget)();
    return a && s.includes(a) ? (0, task_util_js_1.getAlignTarget)() : s.includes(r) ? r : common_const_js_1.DefaultTargetConst.DEFAULT_TARGET;
  }
  static calDepHarTargets(e) {
    const t = new Map(),
      o = e.getModuleService().getModuleModel(),
      r = o.getName(),
      i = e.getTargetData().getTargetName(),
      s = `${r}@${i}`,
      a = hvigor_1.hvigorCore.getCache("depTargetsCache");
    return a.has(s) ? a.get(s) : (e.getModuleService().getHarModuleDependencies().forEach(s => {
      const a = this.getHarTargetData(s, o, i, r, e);
      if (a) t.set(s[0], a);else {
        const r = s[1].getLastDependencyName();
        if (r && t.get(r)) {
          const i = this.getHarTargetData(s, o, t.get(r), r, e);
          i && t.set(s[0], i);
        }
      }
    }), a.set(s, t), t);
  }
  static getHarTargetData(e, t, o, r, i) {
    const s = e[0];
    if (e[1].getLastDependencyName() && e[1].getLastDependencyName() !== r) return "";
    const a = t.getModule().findModuleByName(s);
    if (!a) return this._log.printErrorExit("HAR_TARGET_NOT_MATCH", [o, s], [[s, r]]), "";
    if (i.getModuleService().isFaMode()) return this.getHarTargetName(i, s);
    const n = a.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAR_PLUGIN);
    n || this._log.printErrorExit("MODULE_PLUGIN_NOT_RECEIVE", [s, path_1.resolve(e[1].getDependencyRootPath(), "hvigorfile.ts")], [[s], [s]]);
    const l = n.getNeedExecTargetServiceList();
    l && 0 !== l.length || this._log.printErrorExit("HAR_TARGET_NOT_MATCH", [o, s], [[s, r]]);
    const c = target_utils_js_1.TargetUtils.pushTryTargets({
      projectModel: i.getModuleService().getProjectModel(),
      moduleName: s,
      superTargetName: o
    });
    for (const e of c) {
      if (l.find(t => t.getTargetData().getTargetName() === e)) return e;
    }
    return common_const_js_1.DefaultTargetConst.DEFAULT_TARGET;
  }
  static getDep2ConfigMap(e) {
    if (!e) return;
    const t = e.getModuleService();
    if (!t) return;
    const o = new Map(),
      r = new Map(),
      i = new Map(),
      s = new Map(),
      a = new Map();
    return t.getHarModuleDependencies().forEach(t => {
      var n, l, c;
      const g = this.getDepTargetService(e, t);
      g && (this.collectWorkers(o, null === (n = g.getBuildOption().sourceOption) || undefined === n ? undefined : n.workers, t[1]), this.collectRuntimeOnly(r, i, null === (l = g.getBuildOption().arkOptions) || undefined === l ? undefined : l.runtimeOnly, t[1]), this.collectRouterMapPageSourceFiles(s, g.getTargetRouterMapObjList(g.getModuleService().getModuleModel()), t[1]), this.collectResources(a, null === (c = g.getTargetData().getTargetOpt().resource) || undefined === c ? undefined : c.directories, t[1]));
    }), t.getRemoteHarDependencies().forEach(e => {
      var t, n, l, c;
      if (e.isByteCodeHarDependency()) return;
      const g = e.getPackageJsonObj();
      this.collectWorkers(o, null === (t = g.metadata) || undefined === t ? undefined : t.workers, e), this.collectRuntimeOnly(r, i, null === (n = g.metadata) || undefined === n ? undefined : n.runtimeOnly, e), this.collectRouterMapPageSourceFiles(s, dependency_manager_js_1.DependencyManager.getRemoteDependencyRouterMapObjList(e), e), this.collectResources(a, null === (c = null === (l = g.metadata) || undefined === l ? undefined : l.resource) || undefined === c ? undefined : c.directories, e);
    }), {
      dep2Workers: o,
      dep2RuntimeOnlyObj: {
        dep2RuntimeOnlySources: r,
        dep2RuntimeOnlyPackages: i
      },
      dep2RouterMapObjPageSourceFiles: s,
      dep2Resources: a
    };
  }
  static collectWorkers(e, t, o) {
    (null == t ? undefined : t.length) && e.set(o, t);
  }
  static collectRuntimeOnly(e, t, o, r) {
    o && (o.sources && e.set(r, o.sources), o.packages && t.set(r, o.packages));
  }
  static collectRouterMapPageSourceFiles(e, t, o) {
    if (!(null == t ? undefined : t.length)) return;
    const r = [];
    t.forEach(e => {
      r.push(e.pageSourceFile);
    }), e.set(o, [...r]);
  }
  static collectResources(e, t, o) {
    const r = `${build_directory_const_js_1.BuildDirConst.SRC}${path_1.sep}${build_directory_const_js_1.BuildDirConst.MAIN}${path_1.sep}${build_directory_const_js_1.BuildDirConst.RESOURCES}`;
    e.set(o, (null == t ? undefined : t.length) ? [...t] : [r]);
  }
  static getDepTargetService(e, t) {
    if (!e) return;
    const o = e.getModuleService(),
      r = o.getProjectModel(),
      i = o.getModuleModel(),
      s = HarTargetUtil.getHarTargetData(t, i, e.getTargetData().getTargetName(), i.getName(), e),
      a = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(t[1], r);
    return a ? r.getTarget(a.getName(), s) : undefined;
  }
  static getHarPathToResourcesDirMap(e, t) {
    var o, r;
    HarTargetUtil.calDepHarTargets(t).forEach((o, r) => {
      var i, s, a;
      const n = t.getModuleService().getProjectModel().getModuleModelByName(r);
      if (undefined === n) return;
      const l = n.getProfileOpt();
      if (null === (i = null == l ? undefined : l.targets) || undefined === i ? undefined : i.length) {
        const t = l.targets.find(e => e.name === o);
        if (null === (a = null === (s = null == t ? undefined : t.resource) || undefined === s ? undefined : s.directories) || undefined === a ? undefined : a.length) {
          const o = t.resource.directories.map(e => e.replace(/\\/g, "/"));
          e.set(n.getProjectDir(), file_util_js_1.FileUtil.convertToAbsolutePaths(o, n.getProjectDir()));
        }
      }
    });
    const i = null === (r = null === (o = t.getBuildOption()) || undefined === o ? undefined : o.resOptions) || undefined === r ? undefined : r.excludeHarRes;
    t.getModuleService().getHarDependencies().forEach(o => {
      var r, s, a;
      if (e.has(o.getDependencyRootPath()) || this.isNeedExclude(i, o, t)) return;
      const n = json_util_js_1.JsonUtil.getJson5Obj(o.getPackageFilePath(), "utf-8");
      if (null === (a = null === (s = null === (r = null == n ? undefined : n.metadata) || undefined === r ? undefined : r.resource) || undefined === s ? undefined : s.directories) || undefined === a ? undefined : a.length) {
        const t = n.metadata.resource.directories.map(e => e.replace(/\\/g, "/"));
        e.set(o.getDependencyRootPath(), file_util_js_1.FileUtil.convertToAbsolutePaths(t, o.getDependencyRootPath()));
      } else e.set(o.getDependencyRootPath(), [path_1.resolve(o.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES)]);
    });
  }
  static isNeedExclude(e, t, o) {
    if (!e) return false;
    const r = e.includes(t.getDependencyName()),
      i = t.isLocal(),
      s = !o.getModuleService().getModuleModel().isHarModule();
    return r && !i && s;
  }
}
HarTargetUtil._log = ohos_logger_js_1.OhosLogger.getLogger("HarTargetUtil");
