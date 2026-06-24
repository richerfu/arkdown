/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/ohmurl-utils.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as ohos_logger_js_1 from "./log/ohos-logger";
const log = ohos_logger_js_1.OhosLogger.getLogger("ohmurl-utils"),
  SEPARATOR_BITWISE_AND = "&",
  SEPARATOR_AT = "@",
  SEPARATOR_SLASH = "/";
export function generateOhmUrl(e) {
  return `@normalized:${e.isSO ? "Y" : "N"}&${e.moduleName}&${e.bundleName}&${e.entryPath ? `${e.packageName}/${removeSuffix(e.entryPath)}` : ""}&${e.version}`;
}
export function generateOhmUrlForSourceFile(e, r) {
  return `@normalized:${e.isSO ? "Y" : "N"}&${e.moduleName}&${e.bundleName}&${r ? `${e.packageName}/${removeSuffix(r)}` : ""}&${e.version}`;
}
export function getOldOhmUrlForSrcEntry(e, r, t) {
  var o;
  const n = e.getModuleService().getProjectModel();
  return `@bundle:${null !== (o = e.getTargetData().getProduct().bundleName) && undefined !== o ? o : n.getDefaultBundleName()}/${t}/${removeSuffix(r.replace(/^\.\//, ""))}`;
}
export function generateOhmUrlForSo(e, r) {
  return `@normalized:${e.isSO ? "Y" : "N"}&${e.moduleName}&${e.bundleName}&${r ? path_1.basename(r) : ""}&${e.version}`;
}
export function generateOldOhmUrlForSo(e, r, t) {
  var o;
  const n = e.getModuleService().getProjectModel(),
    l = path_1.basename(r).replace(/^lib/, "");
  return `@app:${null !== (o = e.getTargetData().getProduct().bundleName) && undefined !== o ? o : n.getDefaultBundleName()}/${t}/${removeSuffix(l)}`;
}
export function removeSuffix(e) {
  const r = e.split(path_1.sep).join("/"),
    t = [".d.ets", ".d.ts", ".d.mjs", ".d.cjs", ".d.js", ".ets", ".ts", ".mjs", ".cjs", ".js", ".so"].find(e => r.endsWith(e)),
    o = t ? r.substring(0, r.lastIndexOf(t)) : r;
  return path_1.posix.normalize(o);
}
export function generateOhmurlForSrcEntry(e, r, t, o) {
  var n, l, a;
  let d, i;
  const s = e.getAbsoluteSrcEntryPath(o, t.getDependencyRootPath());
  if (!t.isByteCodeHarDependency() && !fs_extra_1.existsSync(s)) return log.debug("Failed to generate the ohmurl corresponding to the srcEntry that depends on the ability module. The srcEntry path for obtaining the ability of the module on which the module depends is incorrect or the corresponding file does not exist."), "";
  const u = e.getModuleService().getProjectModel(),
    g = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(t, u);
  if (t.isLocal()) {
    if (!g) return log.debug("Failed to generate the ohmurl corresponding to the srcEntry that depends on the ability module. Failed to obtain the corresponding module model through the local dependency of the module."), "";
    i = s.substring(g.getModule().getNodeDir().length + 1);
  } else i = s.substring(t.getDependencyRootPath().length + 1);
  if (null === (l = null === (n = e.getBuildOption()) || undefined === n ? undefined : n.strictMode) || undefined === l ? undefined : l.useNormalizedOHMUrl) d = generateOhmUrlForSourceFile(e.getPkgContextInfo(t.getDependencyName()), i);else {
    const n = null !== (a = e.getTargetData().getProduct().bundleName) && undefined !== a ? a : u.getDefaultBundleName(),
      l = removeSuffix(o.replace(/^\.\//, ""));
    d = t.isLocal() ? `@bundle:${n}/${r}@${g.getModule().getName()}/${l}` : `@bundle:${n}/${r}@${t.getDependencyName()}/${l}`;
  }
  return d;
}
export function transformOhmUrlToPkgName(e) {
  const r = e.split("&");
  if (r.length < 4) throw new Error(`invalid ohmulr ${e}`);
  const t = r[3],
    o = t.split("/");
  return t.startsWith("@") ? o.slice(0, 2).join("/") : o[0];
}
