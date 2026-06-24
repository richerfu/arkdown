/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/remote-hsp-utils.js
 */
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as semver_1 from "semver";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_logger_js_1 from "./log/ohos-logger";
import * as sign_util_js_1 from "../tasks/sign/sign-util";
import * as oh_package_loader_js_1 from "./loader/file/oh-package-loader";
import * as module_task_service_js_1 from "../tasks/service/module-task-service";
const _log = ohos_logger_js_1.OhosLogger.getLogger("remote-hsp-info"),
  getRemoteHspObjList = e => {
    const t = e.getProject(),
      s = path_1.resolve(e.getNodeDir(), common_const_js_1.CommonConst.OH_MODULES, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP),
      o = path_1.resolve(t.getNodeDir(), common_const_js_1.CommonConst.OH_MODULES, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP),
      r = (0, getRemoteHspMap)(s),
      a = (0, getRemoteHspMap)(o, r);
    return a ? [...a.values()] : [];
  };
export const getRemoteHspMap = (e, t) => {
  if (!fs_extra_1.existsSync(e)) return _log.debug("allRemoteHspMap does not exist."), t;
  t && _log.debug(`allRemoteHspMap: ${JSON.stringify((0, hvigor_common_1.strMapToObj)(t))}`);
  const s = null != t ? t : new Map(),
    o = fs_extra_1.readdirSync(e);
  for (const r of o) {
    const o = path_1.resolve(e, r),
      a = (0, getRemoteHspObj)(o, r, _log);
    a && checkIfNeedSet(a, _log, t) && s.set(a.hspFileName, {
      ...a
    });
  }
  return s;
};
const checkIfNeedSet = (e, t, s) => {
    if (!s) return t.debug(`allRemoteHspMap does not exist, ${e.hspFileName} ${e.hspVersion} should be stored in the allRequiredSignRemoteHspMap`), true;
    return s.get(e.hspFileName) ? (0, compareIfSetRemoteHsp)(e, s) ? (t.debug(`allRemoteHspMap exists, ${e.hspFileName} is in allRemoteHspMap, but ${e.hspFileName} ${e.hspVersion} is higher, ${e.hspFileName} ${e.hspVersion} should be stored in the allRequiredSignRemoteHspMap`), true) : (t.debug(`${e.hspFileName} ${e.hspVersion} should not be stored in the allRequiredSignRemoteHspMap`), false) : (t.debug(`allRemoteHspMap exists, but ${e.hspFileName} is not in allRemoteHspMap, ${e.hspFileName} ${e.hspVersion} should be stored in the allRequiredSignRemoteHspMap`), true);
  },
  getRemoteHspObj = (e, t, s) => {
    if (!fs_extra_1.existsSync(e)) return;
    const o = fs_extra_1.readdirSync(e);
    let r, a;
    for (const t of o) if (t.endsWith(build_directory_const_js_1.BuildArtifactExtension.DOT_HSP) && (r && s.printErrorExit("ALLOW_ONE_HSP", [e], [[e]]), r = t), t.includes(common_const_js_1.CommonConst.OH_PACKAGE_JSON5)) {
      const t = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(e, common_const_js_1.CommonConst.OH_PACKAGE_JSON5));
      a = (0, getHspBaseData)(t, s);
    }
    return a ? r ? (a.hspFileName = r, a.hspPath = path_1.resolve(e, r), a.hspDirName = t, a) : void s.printErrorExit("NO_HSP_FOUND", [e], [[e]]) : void s.printErrorExit("NO_PACKAGE_FOUND", [r, e], [[e]]);
  };
export const getHspBaseData = (e, t) => {
  var s;
  if (!fs_extra_1.existsSync(e)) return;
  const o = fs_extra_1.readJsonSync(e);
  return (0, semver_1.valid)(null == o ? undefined : o.version) || t.printErrorExit("VERSION_ILLEGAL", [e]), {
    hspName: o.name,
    hspVersion: o.version,
    hspPath: "",
    hspDirName: "",
    hspFileName: "",
    isIntegratedHsp: !!(null === (s = null == o ? undefined : o.metadata) || undefined === s ? undefined : s.integratedHsp)
  };
};
export const compareIfSetRemoteHsp = (e, t) => {
  if (!t.has(e.hspFileName)) return true;
  const s = t.get(e.hspFileName);
  if (s) {
    return semverMaxSatisfying([e.hspVersion, s.hspVersion]) === e.hspVersion;
  }
  return true;
};
export function semverMaxSatisfying(e) {
  return (0, semver_1.maxSatisfying)(e, "*") || (0, semver_1.maxSatisfying)(e, "*", {
    includePrerelease: true,
    loose: true
  });
}
export function getSignRemoteHspMetadata(e, t) {
  const s = e.getProjectModel(),
    o = [];
  if (!(0, sign_util_js_1.isSigned)(s)) return;
  const r = e.getOhpmRemoteHspAllInfo(e, t, true);
  return Object.entries(r).forEach(([e, t]) => {
    var s, r;
    const a = {
      hspName: t.signedRemoteHspPath ? path_1.basename(t.signedRemoteHspPath) : "",
      hspVersion: t.version,
      hspPath: null !== (s = t.signedRemoteHspPath) && undefined !== s ? s : "",
      soPath: null !== (r = t.soPath) && undefined !== r ? r : ""
    };
    o.push(a);
  }), o;
}
export function getRemoteHspPathMap(e, t) {
  const s = e.getProjectModel(),
    o = new Map(),
    r = new Map(),
    a = s.getRemoteHspPath();
  if (e instanceof module_task_service_js_1.ModuleTaskService) {
    initSignRemoteHspMap(e.getModuleModel().getRemoteHspPath(), r, t);
  }
  return initSignRemoteHspMap(a, r, t), r.forEach(e => {
    const t = e.hspFileName;
    o.set(e.hspName, t);
  }), o;
}
export function initSignRemoteHspMap(e, t, s) {
  if (!fs_extra_1.existsSync(e)) return;
  const o = fs_extra_1.readdirSync(e);
  for (const r of o) {
    const o = path_1.resolve(e, r),
      a = (0, getRemoteHspObj)(o, r, _log),
      i = new Set(s.map(e => e.getPackageName()));
    a && (0, compareIfSetRemoteHsp)(a, t) && i.has(a.hspName) && t.set(a.hspFileName, {
      ...a
    });
  }
}
export { getRemoteHspObj, getRemoteHspObjList };
