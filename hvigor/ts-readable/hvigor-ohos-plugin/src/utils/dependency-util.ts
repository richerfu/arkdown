/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/dependency-util.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../const/common-const";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as file_util_js_1 from "./file-util";
import * as har_target_util_js_1 from "./har-target-util";
import * as parameter_utils_js_1 from "./parameter-utils";
import * as build_mode_manager_js_1 from "../project/build-option/build-mode-manager";
const json5Map = hvigor_1.hvigorCore.getCache("JSON_CACHE");
export function getJson5Obj(e, t = false) {
  var n;
  const o = fs_1.statSync(e).mtime.getTime();
  (null === (n = json5Map.get(e)) || undefined === n ? undefined : n.timeStamp) !== o && json5Map.set(e, {
    timeStamp: o,
    fileContent: hvigor_1.Json5Reader.getJson5Obj(e)
  });
  const r = json5Map.get(e).fileContent;
  return t ? (0, wdk_1.cloneDeep)(r) : r;
}
export function minimizeHspDependencies(e, t, n) {
  (0, parameter_utils_js_1.getPropertyKeepDependency)() || (e.dependencies = getDependenciesWithoutHar(dependency_interface_js_1.DependencyEnum.DEPENDENCIES, t, n), e.dynamicDependencies = getDependenciesWithoutHar(dependency_interface_js_1.DependencyEnum.DYNAMIC_DEPENDENCIES, t, n), e.devDependencies && delete e.devDependencies);
}
export function getDependenciesWithoutHar(e, t, n) {
  const o = {},
    r = t.getHspDependencies().filter(t => t.getDependencyEnum() === e);
  if (0 === r.length) return {};
  const a = {
    parameterFilePath: t.getProjectModel().getParameterFileAbsolutePath(),
    ohPackageFilePath: n
  };
  return r.forEach(e => {
    const n = e.getDependencyName();
    o[n] = file_util_js_1.FileUtil.extracted(e.getDependedVersion(), t.getModuleModel().getParentProject().getParameterFileObj(), a);
  }), o;
}
export function getLocalDepHarTargetSources(e, t, n) {
  var o;
  const r = [],
    a = har_target_util_js_1.HarTargetUtil.calDepHarTargets(e),
    s = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(t, n),
    i = null == s ? undefined : s.getModule().getNodeDir(),
    c = null == s ? undefined : s.getModule().getName(),
    d = c ? a.get(c) : "default",
    l = null == s ? undefined : s.getTargetOptions().filter(e => e.name === d),
    p = l ? null === (o = l[0].resource) || undefined === o ? undefined : o.directories : undefined;
  if ((null == p ? undefined : p.length) && i && p.forEach(e => {
    const t = path_1.resolve(i, e);
    fs_1.existsSync(t) && (null == r || r.push(t));
  }), !(null == p ? undefined : p.length) && i) {
    const e = path_1.resolve(i, common_const_js_1.CommonConst.SRC_MAIN_RESOURCES);
    fs_1.existsSync(e) && (null == r || r.push(e));
  }
  return r;
}
export function getRemoteDepHarTargetSources(e) {
  var t, n;
  const o = [],
    r = e.getDependencyRootPath(),
    a = null === (n = null === (t = getJson5Obj(path_1.resolve(r, common_const_js_1.CommonConst.OH_PACKAGE_JSON5)).metadata) || undefined === t ? undefined : t.resource) || undefined === n ? undefined : n.directories;
  (null == a ? undefined : a.length) && a.forEach(e => {
    const t = path_1.resolve(r, e);
    fs_1.existsSync(t) && (null == o || o.push(t));
  });
  const s = path_1.resolve(r, common_const_js_1.CommonConst.SRC_MAIN_RESOURCES);
  return !o.includes(s) && fs_1.existsSync(s) && (null == o || o.push(s)), o;
}
export function getLocalDependencyRouterMap(e, t, n) {
  const o = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(n, e),
    r = har_target_util_js_1.HarTargetUtil.calDepHarTargets(t),
    a = o && r.get(o.getName());
  return dependency_manager_js_1.DependencyManager.getLocalDependencyRouterMapObjList(n, e, a);
}
export function getLocalDependencyShareConfig(e, t, n) {
  const o = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(n, e),
    r = har_target_util_js_1.HarTargetUtil.calDepHarTargets(t),
    a = o && r.get(o.getName());
  return dependency_manager_js_1.DependencyManager.getLocalDependencyShareConfigObjList(n, e, a);
}
export function getLocalDependencyBuildOptions(e, t) {
  var n, o, r;
  const a = null !== (o = null === (n = e.getModuleJsonObj()) || undefined === n ? undefined : n.module.name) && undefined !== o ? o : e.getPackageName(),
    s = null !== (r = t.get(a)) && undefined !== r ? r : "default";
  return build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(a, s);
}
