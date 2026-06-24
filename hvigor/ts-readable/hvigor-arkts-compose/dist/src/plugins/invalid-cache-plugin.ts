/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/invalid-cache-plugin.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as arkts_pack_js_1 from "../arkts-pack";
import * as constants_js_1 from "../util/constants";
import * as utils_js_1 from "../util/utils";
import * as module_meta_info_plugin_js_1 from "./module-meta-info-plugin";
let allPlugins;
const OLD_HAR_NAME_OHM_MAP = "oldHarNameOhmMap",
  CUR_HAR_NAME_OHM_MAP = "curHarNameOhmMap",
  PKG_JSON_FILE_HASH = "pkgJsonFileHash",
  NEW_OHM_URL_HASH = "newOhmUrlHash",
  ROUTER_MAP_HASH = "routerMapHash",
  CASE_SENSITIVE_CHECK = "caseSensitiveCheck",
  MODULE_DEPS = "moduleDeps",
  PROJECT_ARK_OPTION = "projectArkOption",
  TS_IMPORT_SENDABLE = "tsImportSendable",
  PROJECT_OH_PACKAGE = "projectDevDept",
  OHOS_UI_TRANSFORM_OPTIMIZATION = "ohos.uiTransform.Optimization",
  BUNDLE_NAME = "bundleName",
  IS_BYTE_CODE_HAR_OPTIMIZE = "isByteCodeHarOptimize",
  PRELOAD_SO_FILE_SET = "preloadSoFileSet",
  depCache = new Map();
let inCIEnv = false,
  isPreloadSoFileSetChanged = false;
export function invalidCachePlugin() {
  return {
    name: "invalidCachePlugin",
    buildStart: buildStart,
    shouldTransformCachedModule: shouldTransformCachedModule,
    buildEnd: buildEnd,
    beforeBuild: beforeBuild,
    beforeBuildEnd: beforeBuildEnd,
    cleanUp() {
      depCache.clear();
    }
  };
}
function comparePreloadFileSet(e) {
  const t = e.share.projectConfig;
  if (!(0, utils_js_1.allowToPreloadSo)(t)) return false;
  const a = t?.needPreloadSoFileSet || new Set(),
    o = e.cache.get("preloadSoFileSet") || [];
  if (o.length !== a.size) return true;
  for (const e of o) if (!a.has(e)) return true;
  return false;
}
function setProjectConfigCache(e) {
  const t = e.share?.projectConfig,
    a = t?.projectArkOption?.tscConfig?.targetESVersion;
  a && e.cache.set("projectArkOption", a), t?.routerMap && e.cache.set("routerMapHash", t?.routerMap), e.cache.set("oldHarNameOhmMap", e.cache.get("curHarNameOhmMap")), e.cache.delete("curHarNameOhmMap"), e.cache.set("newOhmUrlHash", !!t?.useNormalizedOHMUrl), e.cache.set("caseSensitiveCheck", t?.caseSensitiveCheck), e.cache.set("tsImportSendable", t?.tsImportSendable), e.cache.set("isByteCodeHarOptimize", t?.isByteCodeHarOptimize ?? false);
}
function buildEnd() {
  setProjectConfigCache(this);
  const e = new Map();
  for (const t of this.getModuleIds()) {
    const a = this.getModuleInfo(t);
    e.set(t, a ? a.importedIds.concat(a.dynamicallyImportedIds) : []);
  }
  this.cache.set(MODULE_DEPS, e);
  const t = this.share?.projectConfig,
    a = t?.mockParams?.mockConfigPath;
  if (a && fs_extra_1.existsSync(a)) {
    const e = (0, hvigor_common_1.hash)(fs_extra_1.readFileSync(a));
    this.cache.set(a, e);
  }
  (0, utils_js_1.allowToPreloadSo)(t) ? this.cache.set("preloadSoFileSet", [...(t?.needPreloadSoFileSet || [])]) : this.cache.delete("preloadSoFileSet");
}
function beforeBuildEnd() {
  const e = this.share?.projectConfig;
  e?.pkgJsonFileHash && this.cache.set("pkgJsonFileHash", e?.pkgJsonFileHash), setUserConfigCache(this), this.cache.set("projectDevDept", this.share?.projectDevDept), this.cache.set(OHOS_UI_TRANSFORM_OPTIMIZATION, this.share?.projectConfig?.uiTransformOptimization), this.cache.set(BUNDLE_NAME, this.share?.projectConfig?.bundleName);
}
function buildStart({
  plugins: e
}) {
  const t = this.share?.projectConfig?.mockParams?.mockConfigPath;
  if (t && fs_extra_1.existsSync(t)) {
    const e = this.cache?.get(t),
      a = (0, hvigor_common_1.hash)(fs_extra_1.readFileSync(t));
    this.share.projectConfig.mockParams.isMockConfigChanged = a !== e;
  } else this.share?.projectConfig?.mockParams && (this.share.projectConfig.mockParams.isMockConfigChanged = false);
  inCIEnv = (0, hvigor_common_1.isCI)(), isPreloadSoFileSetChanged = comparePreloadFileSet(this);
  const a = (0, arkts_pack_js_1.loadAceBuildJson)(this.share?.projectConfig?.aceBuildJson).hspNameOhmMap;
  this.cache.set("curHarNameOhmMap", a), allPlugins = [], allPlugins.push(...e), allPlugins = allPlugins.filter(e => "object" == typeof e && (e.shouldInvalidCache ?? false));
}
function beforeBuild() {
  if (this.share?.projectConfig?.projectTopDir) {
    const e = path_1.resolve(this.share.projectConfig.projectTopDir, "oh-package.json5"),
      t = fs_extra_1.readFileSync(e, "utf-8");
    this.share.projectDevDept = (0, hvigor_common_1.parseJsonText)(t)?.devDependencies || {};
  }
}
function setUserConfigCache(e) {
  e.share?.projectConfig?.userProjectConfig && Object.entries(e.share?.projectConfig?.userProjectConfig).forEach(([t, a]) => {
    e.cache.set(t, a);
  });
}
function preCheckShouldTransform(e) {
  let t;
  e.share?.projectConfig?.userProjectConfig && (t = Object.entries(e.share?.projectConfig?.userProjectConfig).some(([t, a]) => !(0, wdk_1.isEqual)(e.cache.get(t), a)));
  const a = e.share?.projectConfig?.projectArkOption?.tscConfig?.targetESVersion !== e.cache.get("projectArkOption"),
    o = e.share?.projectConfig?.tsImportSendable !== !!e.cache.get("tsImportSendable");
  return t || a || o;
}
function isHarNameOhmChanged(e) {
  const t = e.cache.get("oldHarNameOhmMap"),
    a = e.cache.get("curHarNameOhmMap");
  if (t && a) for (const e of Object.keys(a)) if (t[e] && a[e] !== t[e]) return true;
  return false;
}
function isByteCodeHarOptimizeChanged(e) {
  return (e.cache.get("isByteCodeHarOptimize") ?? false) !== (e.share?.projectConfig?.isByteCodeHarOptimize ?? false);
}
function isPkgJsonFileChanged(e) {
  return e.share?.projectConfig?.pkgJsonFileHash !== e.cache.get("pkgJsonFileHash");
}
async function isPluginsHasInvalidCache(e, t) {
  const a = await Promise.all(allPlugins.map(a => a.shouldInvalidCache.apply(e, [t])));
  let o = false;
  return a.forEach(e => {
    e && "boolean" == typeof e && (o || (o = e));
  }), o;
}
async function shouldTransformCachedModule(e) {
  if (localTestHasChangedMockConfig(this)) return true;
  if (!(0, wdk_1.isEqual)(this.cache?.get("projectDevDept") ?? {}, this.share?.projectDevDept ?? {})) return true;
  if (isPkgJsonFileChanged(this) && (inCIEnv || !this.share?.projectConfig?.resolveConflictMode)) return true;
  if (preCheckShouldTransform(this)) return true;
  if (isHarNameOhmChanged(this)) return true;
  if (isModulePkgNameChanged(e)) return true;
  if (bundleNameHasChanged(this)) return true;
  if (isByteCodeHarOptimizeChanged(this)) return true;
  if (isPreloadSoFileSetChanged) return true;
  return (await isPluginsHasInvalidCache(this, e)) || hasHookEventChangedDeps(this, e);
}
function localTestHasChangedMockConfig(e) {
  return !(!e?.share?.projectConfig?.isLocalTest || !e.share.projectConfig.mockParams?.isMockConfigChanged);
}
function bundleNameHasChanged(e) {
  return e.share?.projectConfig?.uiTransformOptimization !== e.cache.get(OHOS_UI_TRANSFORM_OPTIMIZATION) || false === e.share?.projectConfig?.uiTransformOptimization && e.cache.get(BUNDLE_NAME) !== e.share?.projectConfig?.bundleName;
}
function hasHookEventChangedDeps(e, t) {
  const a = e.share.getHookEventFactory("invalidCachePlugin", "shouldTransformCachedModule"),
    o = a.createEvent("hasChangedDeps")?.start(),
    i = hasChangedDeps(e.cache.get(MODULE_DEPS)?.get(t.id), e.share.depInfo.changedDeps);
  return o?.stop(), i;
}
function hasChangedDeps(e, t) {
  if (!e) return false;
  for (const a of e) {
    if (!depCache.has(a)) {
      const e = depChanged(a, t);
      depCache.set(a, e);
    }
    if (depCache.get(a)) return true;
  }
  return false;
}
function depChanged(e, t) {
  let a = false;
  for (const {
    name: o,
    lastRootPath: i
  } of t) if (i && e.startsWith(i) || e === o || e.startsWith(o)) {
    a = true;
    break;
  }
  return a;
}
function isModulePkgNameChanged(e) {
  if (!(e?.meta?.pkgPath && e?.meta?.pkgName)) return false;
  const t = (0, module_meta_info_plugin_js_1.getPackageJsonInfo)(path_1.resolve(e.meta.pkgPath, constants_js_1.OH_PACKAGE_JSON5));
  return e.meta.pkgName !== t?.pkgName;
}
