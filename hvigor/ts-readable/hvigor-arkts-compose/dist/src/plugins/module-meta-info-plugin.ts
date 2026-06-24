/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/module-meta-info-plugin.js
 */
import path_1 from "path";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as fs from "fs-extra";
import * as constants_js_1 from "../util/constants";
import * as path_cache_stat_js_1 from "../util/path-cache-stat";
import * as utils_js_1 from "../util/utils";
import * as index_js_1 from "./node-resolve/index";
const LOCAL_MODULE_INFO_CACHE_KEY = "localModuleInfoCache",
  CACHE_KEYS = ["allModuleNameHash", "pkgJsonFileHash"],
  OBJECT_ENTRY_KEYS = ["import", ".", "default", "require", "node"],
  NODE_ENTRY_FILE_KEYS = ["exports", "module", "jsnext:main", "main"],
  COMMONJS_ES_IMPORT_SUFFIX = "?commonjs-es-import",
  LOGGER = hvigor_arkts_base_1.ArktsBaseLogger.getPluginLogger("ArkTS import check");
let packageInfoCache,
  localModuleInfoCache,
  packageJsonCache = {},
  dynamicImport = [];
const loadMetaInfoSet = new Set();
let throwErrorFunction, supportExtensions, projectModel, projectTopDir;
const hostModulesInfoCache = new Map();
export function moduleMetaInfoPlugin(e) {
  return supportExtensions = e, {
    name: "moduleInfoMetaPlugin",
    beforeBuild: beforeBuild,
    buildStart: buildStart,
    load: load,
    moduleParsed: moduleParsed,
    beforeBuildEnd: beforeBuildEnd,
    afterBuildEnd: afterBuildEnd,
    cleanUp: cleanUp,
    resolveDynamicImport: resolveDynamicImport,
    resolveId: resolveId
  };
}
function resolveDynamicImport(e, o) {
  dynamicImport.push(e + o);
}
function resolveId(e, o) {
  if (undefined !== e && undefined !== o && !dynamicImport.includes(e + o)) try {
    const t = this.getModuleInfo(o);
    let n = "",
      a = "";
    t?.meta.belongModulePath && (n = path_1.resolve(t?.meta.belongModulePath, constants_js_1.OH_PACKAGE_JSON5)), t?.meta.belongProjectPath && (a = path_1.resolve(t?.meta.belongProjectPath, constants_js_1.OH_PACKAGE_JSON5));
    let r = {};
    if (path_cache_stat_js_1.PathCacheStat.exists(n, this.share.getCachedFileInfo)) {
      const e = getJsonObject(n);
      e && e.dynamicDependencies && (r = e.dynamicDependencies);
    }
    if (path_cache_stat_js_1.PathCacheStat.exists(a, this.share.getCachedFileInfo)) {
      const e = getJsonObject(a);
      e && e.dynamicDependencies && (r = {
        ...r,
        ...e.dynamicDependencies
      });
    }
    Object.keys(r).find(o => o === e) && LOGGER.warn(`It is recommended to use dynamic imports for packages listed in dynamicDependencies. Using '${e}' at file: ${o}`);
  } catch (e) {
    LOGGER.debug("oh-package.json5 parse failed:", e.message);
  }
}
function buildStart() {
  loadMetaInfoSet.clear(), packageInfoCache = {}, dynamicImport = [], packageJsonCache = {};
  const e = shouldLoadCache(this.share?.projectConfig, this.cache);
  localModuleInfoCache = e ? this.cache.get("localModuleInfoCache") ?? {} : {}, throwErrorFunction = this.share?.throwArkTsCompilerError, projectModel = this.share?.projectConfig?.projectModel, projectModel || throwError("parsed module meta info error: current projectModel is undefined"), projectTopDir = this.share?.projectConfig?.projectTopDir, projectTopDir || throwError("parsed module meta info error: projectTopDir is undefined");
}
function beforeBuild() {
  this.share?.projectConfig?.sourceMapDir && ("har" === this.share?.projectConfig?.moduleType && "Release" === this.share?.projectConfig?.buildMode || "har" !== this.share?.projectConfig?.moduleType && "Debug" === this.share?.projectConfig?.buildMode) && fs.ensureDirSync(this.share?.projectConfig?.sourceMapDir);
}
function getPackageInfoUseCachedPackageJsonPath(e) {
  if (!index_js_1.idToPackageInfo) return;
  if (!index_js_1.idToPackageInfo.has(e)) return;
  const o = index_js_1.idToPackageInfo.get(e);
  if (!o) return;
  const t = "string" == typeof o ? o : o.packageJsonPath;
  if (!t) return;
  let n = getJsonObject(t);
  if (!n) return;
  const a = getNodeEntryFiles(t, n);
  return a ? (packageInfoCache[e] = {
    pkgPath: path_1.dirname(t),
    nodeEntryFiles: a,
    pkgName: n.name,
    pkgVersion: n.version
  }, packageInfoCache[e]) : undefined;
}
function addModuleInfoData(e, o, t) {
  const n = e.includes(constants_js_1.OH_NODE_MODULES) || e.includes(constants_js_1.NODE_MODULES) ? loadRemoteDependencyInfo(e, this.share?.projectConfig?.projectTopDir) : loadLocalDependencyInfo(e, o);
  let a;
  if (!(0, hvigor_common_1.isChildPath)(e, this.share.projectConfig.modulePath)) {
    const e = this.share?.projectConfig?.localModuleParamMap;
    a = {
      pkgName: n.pkgName,
      pkgVersion: e && e.has(n.pkgPath) ? e.get(n.pkgPath) : n.pkgVersion
    };
  }
  t.meta ? Object.assign(t.meta, n, {
    dependencyPkgInfo: a
  }) : t.meta = {
    ...n,
    dependencyPkgInfo: a
  }, undefined !== this.share.projectConfig.resourceTableTs && undefined !== this.share.projectConfig.moduleMetaInfoFiles && path_1.normalize(e) === path_1.normalize(this.share.projectConfig.resourceTableTs) && (t.meta.ohmurl = this.share.projectConfig.moduleMetaInfoFiles.filter(e => e.filePath === this.share.projectConfig.resourceTableTs)[0].ohmurl);
}
function load(e) {
  const o = this.getModuleInfo(e);
  if (!o) return;
  const t = o.meta?.isHarDeduplicate && e.startsWith("\0");
  if (t && (e = e.substring(1)), loadMetaInfoSet.has(e)) return;
  let n = false;
  (0, utils_js_1.isVirtualFile)(e) && (n = e.includes("?commonjs-es-import"), !n) || o.isExternal && !t || (addModuleInfoData.call(this, e, n, o), loadMetaInfoSet.add(e));
}
function moduleParsed(e) {
  for (const o of e?.importedIds ?? []) load.call(this, o);
  for (const o of e?.dynamicallyImportedIds ?? []) load.call(this, o);
  e && dealHostModuleInfo(e.meta.moduleName, e, this.getModuleInfo);
}
function processNeedPreloadSoTag(e, o, t) {
  const n = e.share?.projectConfig?.needPreloadSoFileSet;
  if (!n?.size) return;
  const a = [...n];
  for (; a.length;) {
    const n = a.pop(),
      r = e.getModuleInfo(n);
    !r || r.isExternal || o(r) || (t(r), a.push(...r.importedIds, ...r.dynamicallyImportedIds));
  }
}
function addNeedPreloadSoTag(e) {
  processNeedPreloadSoTag(e, e => !!e.meta?.needPreloadSo, e => {
    e.meta = {
      ...e.meta,
      needPreloadSo: true
    };
  });
}
function removeNeedPreloadSoTag(e) {
  processNeedPreloadSoTag(e, e => !e.meta?.needPreloadSo, e => {
    e.meta?.needPreloadSo && delete e.meta.needPreloadSo;
  });
}
function beforeBuildEnd() {
  "true" === this.share?.projectConfig?.watchMode && loadMetaInfoSet.clear(), this.cache.set("localModuleInfoCache", localModuleInfoCache);
  for (const e of CACHE_KEYS) this.cache.set(e, this.share?.projectConfig[e]);
  (0, utils_js_1.allowToPreloadSo)(this.share?.projectConfig) && addNeedPreloadSoTag(this);
}
function afterBuildEnd() {
  (0, utils_js_1.allowToPreloadSo)(this.share?.projectConfig) && removeNeedPreloadSoTag(this);
}
function cleanUp() {
  loadMetaInfoSet.clear(), hostModulesInfoCache.clear(), localModuleInfoCache = null, packageInfoCache = null, dynamicImport = [], packageJsonCache = {};
}
function loadRemoteDependencyInfo(e, o) {
  e.startsWith("\0") && (e = e.substring(1));
  const t = getPackageInfoUseCachedPackageJsonPath(e) ?? getPackageJsonInfo(path_1.resolve(e, ".."));
  return t ? {
    isLocalDependency: false,
    isNodeEntryFile: isNodeEntryFile(e, t.nodeEntryFiles),
    pkgPath: t.pkgPath,
    pkgName: t.pkgName,
    pkgVersion: t.pkgVersion,
    belongProjectPath: o
  } : (throwError(`parsed module meta info error: can not parsed remote dependency ${e} package.json info`), {});
}
function loadLocalDependencyInfo(e, o = false) {
  let t = e;
  if (o && (t = e.substring(1, e.indexOf("?commonjs-es-import"))), localModuleInfoCache[e]) return localModuleInfoCache[e];
  for (const [o, {
    moduleName: n,
    modulePkgPath: a,
    belongProjectPath: r,
    isBuildCacheDir: s
  }] of Object.entries(projectModel)) if ((0, hvigor_common_1.isChildPath)(t, o)) {
    const t = getPackageInfoUseCachedPackageJsonPath(e) ?? getPackageJsonInfo(a);
    return localModuleInfoCache[e] = {
      moduleName: n,
      isLocalDependency: true,
      isNodeEntryFile: false,
      pkgPath: s ? o : a,
      belongProjectPath: s ? path_1.dirname(o) : r
    }, t && Object.assign(localModuleInfoCache[e], {
      pkgName: t.pkgName,
      pkgVersion: t.pkgVersion
    }), localModuleInfoCache[e];
  }
  const n = getPackageJsonInfo(projectTopDir);
  return localModuleInfoCache[e] = {
    moduleName: path_1.basename(projectTopDir),
    isLocalDependency: true,
    isNodeEntryFile: !!n && isNodeEntryFile(e, n.nodeEntryFiles),
    pkgPath: projectTopDir
  }, localModuleInfoCache[e];
}
function shouldLoadCache(e, o) {
  if (!e || !o) return false;
  let t = true;
  for (const n of CACHE_KEYS) {
    if ((e[n] ?? "") !== (o.get(n) ?? "")) {
      t = false;
      break;
    }
  }
  return t;
}
export function getPackageJsonInfo(e) {
  if (e === path_1.dirname(projectTopDir) || e === path_1.dirname(e)) return;
  if (packageInfoCache[e]) return packageInfoCache[e];
  const o = findPackageJsonFile(e);
  if (o) {
    const t = getJsonObject(o),
      n = t ? getNodeEntryFiles(o, t) : undefined;
    return packageInfoCache[e] = n ? {
      pkgPath: e,
      nodeEntryFiles: n,
      pkgName: t.name,
      pkgVersion: t.version
    } : undefined, packageInfoCache[e];
  }
  return packageInfoCache[e] = getPackageJsonInfo(path_1.dirname(e)), packageInfoCache[e];
}
function dealHostModuleInfo(e, o, t) {
  if (e) for (const [n, a] of Object.entries(o.importedIdMaps)) {
    const o = t(a);
    if (!o) continue;
    const r = getModulesInfoCache(a, o),
      s = `${e}:${n}`;
    r.has(s) || (o.meta.hostModulesInfo || (o.meta.hostModulesInfo = []), o.meta.hostModulesInfo.push({
      hostDependencyName: n,
      hostModuleName: e
    }), r.set(s, true));
  }
}
function getModulesInfoCache(e, o) {
  let t = hostModulesInfoCache.get(e);
  if (!t) {
    if (t = new Map(), o.meta.hostModulesInfo) for (const e of o.meta.hostModulesInfo) t.set(`${e.hostModuleName}:${e.hostDependencyName}`, true);
    hostModulesInfoCache.set(e, t);
  }
  return t;
}
function getNodeEntryFiles(e, o) {
  return findNodeEntryFiles(o).map(o => path_1.resolve(e, `../${o}`));
}
export function findNodeEntryFiles(e) {
  const o = [];
  for (const t of NODE_ENTRY_FILE_KEYS) {
    const n = parseEntryFiles(e[t]);
    if (n) {
      o.push(...n);
      break;
    }
  }
  return o.push("index.js"), o;
}
function isNodeEntryFile(e, o) {
  if (-1 !== o.indexOf(e)) return true;
  for (const t of supportExtensions) if (o + t === e) return true;
  return false;
}
function parseEntryFiles(e) {
  if (!e) return;
  if ("string" == typeof e) return [e];
  const o = [];
  if (Array.isArray(e)) for (const t of e) {
    const e = parseEntryFiles(t);
    e && o.push(...e);
  }
  if (o.length > 0) return o;
  if ("object" == typeof e) for (const t of OBJECT_ENTRY_KEYS) {
    const n = parseEntryFiles(e[t]);
    n && o.push(...n);
  }
  return o;
}
function getJsonObject(e) {
  if (packageJsonCache[e]) return packageJsonCache[e];
  try {
    const o = fs.readFileSync(e, "utf-8"),
      t = (0, hvigor_common_1.parseJsonText)(o);
    return packageJsonCache[e] = t, t;
  } catch (e) {
    return;
  }
}
function findPackageJsonFile(e) {
  return findFile((0, utils_js_1.getOhPackageJsonPath)(path_1.join(e, constants_js_1.OH_PACKAGE_JSON5)));
}
function findFile(e) {
  return fs.existsSync(e) ? e : undefined;
}
function throwError(e) {
  if (!throwErrorFunction) throw new Error(e);
  throwErrorFunction(e);
}
