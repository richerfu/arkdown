/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/index.js
 */
import fs_1 from "fs";
import * as path_1 from "path";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as arkts_pack_js_1 from "../../arkts-pack";
import * as constants_js_1 from "../../util/constants";
import * as fs_js_1 from "../../util/fs";
import * as log_js_1 from "../../util/log";
import * as har_deduplicate_util_js_1 from "../har-deduplicate/har-deduplicate-util";
import import_check_plugin_js_1 from "../import-check-plugin";
import * as cache_js_1 from "./cache";
import deprecated_options_js_1 from "./deprecated-options";
import * as resolve_import_specifiers_js_1 from "./resolve-import-specifiers";
import * as resolve_ohmodules_js_1 from "./resolve-ohmodules";
import * as util_js_1 from "./util";
import * as path_cache_stat_js_1 from "../../util/path-cache-stat";
const ES6ImportExportRegExp = /(?:^\s*|[}{\(\);,\n]\s*)(import[\s+|{]['"]|(import|module)[\s+|*|{][^"'\(\)\n;]+[\s|}]+from\s*['"]|export[\s+|{](\*|\{|default|function|var|const|let|[_$a-zA-Z\xA0-\uFFFF][_$a-zA-Z0-9\xA0-\uFFFF]*))/,
  ES6AliasRegExp = /(?:^\s*|[}{\(\);,\n]\s*)(export\s*\*\s*from\s*(?:'([^']+)'|"([^"]+)"))/,
  logger = hvigor_arkts_base_1.ArktsBaseLogger.getLogger("caseSensitiveCheck"),
  _log = hvigor_arkts_base_1.ArktsBaseLogger.getHvigorConsoleLogger("node-resolve");
function isModule(e) {
  return ES6ImportExportRegExp.test(e) || ES6AliasRegExp.test(e);
}
const deepFreeze = e => {
    Object.freeze(e);
    for (const t of Object.values(e)) "object" != typeof t || Object.isFrozen(t) || deepFreeze(t);
    return e;
  },
  baseConditions = ["default", "module"],
  baseConditionsEsm = [...baseConditions, "import"],
  baseConditionsCjs = [...baseConditions, "require"],
  defaults = {
    dedupe: [],
    extensions: [".mjs", ".js", ".json", ".node"],
    resolveOnly: [],
    moduleDirectories: ["oh_modules"],
    ignoreSideEffectsForRoot: false
  },
  ignoreVirtualFilePrefix = "\0hvigor_ignore_";
let harDuplicateFileSet;
export const importee2filePath = new Map();
const allowPatterns = e => {
  const t = e.map(e => {
    if (e instanceof RegExp) return e;
    const t = e.replace(/[\\^$*+?.()|[\]{}]/g, "\\$&");
    return new RegExp(`^${t}$`);
  });
  return e => !t.length || t.some(t => t.test(e));
};
async function updateLocation(e, t, r, s, o, i) {
  if (e && !o) {
    (await (0, fs_js_1.fileExists)(t)) && (t = await (0, fs_js_1.realpathFS)(t));
  }
  return r ? i.set(t, r) : i.set(t, s), t;
}
function initData(e) {
  const t = (0, util_js_1.getMainFields)(e),
    r = true === e.preferBuiltins || false === e.preferBuiltins;
  return {
    conditionsEsm: [...baseConditionsEsm, ...(e.exportConditions || [])],
    conditionsCjs: [...baseConditionsCjs, ...(e.exportConditions || [])],
    packageInfoCache: new Map(),
    idToPackageInfo: new Map(),
    mainFields: t,
    useBrowserOverrides: -1 !== t.indexOf("browser"),
    isPreferBuiltinsSet: r,
    preferBuiltins: !r || e.preferBuiltins,
    rootDir: (0, path_1.resolve)(e.rootDir || process.cwd()),
    dedupe: e.dedupe
  };
}
function reject(e, t) {
  return !!(0, util_js_1.normalizeInput)(t.input).includes(e) && null;
}
function getImportSpecifierList(e, t, r) {
  const s = [e];
  if (undefined !== t || e[0].match(/^\.?\.?\//) || s.push(`./${e}`), t && e.endsWith(".js")) for (const o of [".ts"]) t.endsWith(o) && r.includes(o) && s.push(e.replace(/.js$/, o));
  return s;
}
function addSourceRootsIntoImportSpecifiers(e, t, r, s, o, i, a, n) {
  if (e || !o || !a) return;
  const l = t.getModuleInfo(o)?.meta,
    c = l?.pkgName,
    u = l?.pkgPath,
    d = a[c]?.sourceRoots;
  c && u && r === c && d?.length && d.forEach(e => {
    const t = path_1.default.resolve(u, e, s.join("/"));
    let r = path_1.default.relative(i, t).replace(/\\/g, "/");
    "." !== r.charAt(0) && (r = `./${r}`), n.push(r);
  });
}
function addResolvedMeta(e, t, r, s, o) {
  if (e && s && s > o) {
    const i = r.slice(o + 1, s).map(e => path_1.default.resolve(t, e));
    e.meta = {
      backupSourceRoots: i,
      resolvedBySourceRoot: true
    };
  }
  return e;
}
async function getRes(e, t, r, s, o, i, a, n, l, c, u, d, p) {
  if (e) {
    if (t && c) return !u && r && s !== o && i.warn(`preferring built-in module '${o}' over local alternative at '${r.location}', pass 'preferBuiltins: false' to disable this behavior or 'preferBuiltins: true' to disable this warning`), false;
    if (d && 0 !== a.indexOf((0, path_1.normalize)(d.trim(path_1.sep)))) return null;
  }
  if (p.modulesOnly && (await (0, fs_js_1.fileExists)(a))) {
    return isModule(await (0, fs_js_1.readFile)(a, "utf-8")) ? {
      id: `${a}${n}`,
      moduleSideEffects: l(a)
    } : null;
  }
  return {
    id: `${a}${n}`,
    moduleSideEffects: l(a)
  };
}
function updateBrowserMapCache(e, t, r) {
  if (e) {
    if (Object.prototype.hasOwnProperty.call(e, t)) {
      if (!e[t]) return r.set(t, e), [{
        id: constants_js_1.ES6_BROWSER_EMPTY
      }, null];
      t = e[t];
    }
    r.set(t, e);
  }
  return [null, t];
}
function getResolveLikeNode(e, t, r, s, o, i, a, n, l, c, u, d, p, f, h, _, g, m, j, v, E, S, C) {
  return async (k, x, R, w) => {
    const [P, F] = x.split("?"),
      D = "" + (F ? `?${F}` : "");
    x = P;
    const M = !R || e(x) ? t : (0, path_1.dirname)(R),
      b = r.get(R);
    if (s && b) {
      const e = (0, path_1.resolve)(M, x);
      if (false === b[x] || false === b[e]) return {
        id: constants_js_1.ES6_BROWSER_EMPTY
      };
      x = ("." !== x[0] && b[x] || b[e] || b[`${e}.js`] || b[`${e}.json`]) ?? x;
    }
    const I = x.split(/[/\\]/);
    let y = I.shift(),
      O = false;
    if ("@" === y[0] && I.length > 0 ? y += `/${I.shift()}` : "." === y[0] && (y = (0, path_1.resolve)(M, x), O = true), !O && !o(y)) return reject(x, i);
    const B = getImportSpecifierList(x, R, a),
      $ = B.length - 1;
    addSourceRootsIntoImportSpecifiers(O, k, y, I, R, M, v.pkgNameToPkgInfo, B);
    const T = w && w["node-resolve"] && w["node-resolve"].isRequire,
      q = T ? n : l;
    let A;
    s && !q.includes("browser") && q.push("browser"), A = C && S && E ? await doNativeResolve(T ? S : E, B, M, R) : await (0, resolve_import_specifiers_js_1.resolveImportSpecifiers)({
      importer: R,
      importSpecifierList: B,
      exportConditions: q,
      warn: (...e) => k.warn(...e),
      packageInfoCache: c,
      extensions: a,
      mainFields: u,
      preserveSymlinks: d,
      useBrowserOverrides: s,
      baseDir: M,
      moduleDirectories: p,
      modulePaths: f,
      rootDir: t,
      ignoreSideEffectsForRoot: h,
      caseSensitiveCheck: v.caseSensitiveCheck,
      sdkModulesPath: v.sdkModulesPath
    });
    const H = (0, hvigor_common_1.isBuiltinModule)(x),
      N = H && _ ? {
        packageInfo: undefined,
        hasModuleSideEffects: () => null,
        hasPackageEntry: true,
        packageBrowserField: false
      } : A;
    if (!N) return null;
    const {
      packageJsonPath: L,
      packageInfo: W,
      hasModuleSideEffects: V,
      hasPackageEntry: z,
      packageBrowserField: J
    } = N;
    let {
      location: Y
    } = N;
    const G = updateBrowserMapCache(J, Y, r);
    if (G[0]) return G[0];
    Y = G[1], Y = await updateLocation(z, Y, L, W, d, g);
    return addResolvedMeta(await getRes(z, H, A, N, x, k, Y, D, V, _, m, j, v), M, B, N.resolvedSpecifierIndex, $);
  };
}
async function doNativeResolve(e, t, r, s) {
  for (let o = 0; o < t.length; o++) {
    const i = await e.async(r, t[o]);
    if (i.error || !i.path) continue;
    i.caseSensitiveDetected && s && (0, log_js_1.hvigorConsoleLoggerPrintError)(_log, "CASE_SENSITIVE_DETECTED_AND_IMPORTER", [t[o] + (i.indexLoaded ? "/index" : ""), s]);
    const a = "\\\\?\\";
    return {
      location: i.path.startsWith(a) ? i.path.substring(a.length) : i.path,
      packageJsonPath: i.packageJsonPath,
      resolvedSpecifierIndex: o,
      hasModuleSideEffects: () => null
    };
  }
}
function checkModuleDirectories(e) {
  if (e.some(e => e.includes("/"))) throw new Error("`moduleDirectories` option must only contain directory names. If you want to load modules from somewhere not supported by the default module resolution algorithm, see `modulePaths`.");
}
function updateDedupe(e, t) {
  return "function" != typeof e && (e = e => t.dedupe.includes(e) || t.dedupe.includes((0, util_js_1.getPackageName)(e))), e;
}
function getResolveOnly(e) {
  return "function" == typeof e.resolveOnly ? e.resolveOnly : allowPatterns(e.resolveOnly);
}
function doWidgetCompileCheck(e, t, r) {
  if ("true" === e.widgetCompile && e.byteCodeHar2HspDep?.get(t)?.length) {
    const s = e.byteCodeHar2HspDep?.get(t)?.map(e => `'${e}'`);
    (0, log_js_1.hvigorConsoleLoggerPrintError)(_log, "REMOVE_REFERENCES_FOR_SERVICE_WIDGET", [s.join(", "), r ? `At file: ${r}` : ""]);
  }
}
export function isExternal(e, t, r, s, o = {}) {
  for (const [e, i] of t) if (e.test(r)) return "true" === o.widgetCompile && (0, log_js_1.hvigorConsoleLoggerPrintError)(_log, "REMOVE_REFERENCES_FOR_SERVICE_WIDGET", [i, s ? `At file: ${s}` : ""]), true;
  for (const [t, i] of e) if (t.test(r)) return doWidgetCompileCheck(o, i, s), true;
  return false;
}
export function isBackslashExternal(e, t, r) {
  for (const [e, s] of t) if (e.test(r)) return true;
  for (const [t, s] of e) if (t.test(r)) return true;
  return false;
}
function readSystemApi(e, t) {
  const r = new Map();
  return e.forEach(e => {
    fs_1.readdirSync(e).forEach(s => {
      const o = path_1.default.join(e, s);
      path_cache_stat_js_1.PathCacheStat.isFile(o, t) && r.set(s.replace(".d.ts", "").replace(".d.ets", ""), o);
    });
  }), r;
}
function getVirtualFileId(e) {
  return "\0hvigor_ignore_" + e.replace(/\\/g, "_").replace(/\//g, "_");
}
function loadNativeResolver(e, t, r, s, o, i) {
  if (!e) return [undefined, undefined];
  try {
    const {
        ResolverFactory: e
      } = require("oxc-resolver"),
      a = new e({
        conditionNames: t.conditionsEsm,
        mainFields: t.mainFields,
        extensions: r,
        symlinks: !s,
        modules: [...o, "oh_modules"],
        caseSensitive: i
      }),
      n = a.cloneWithOptions({
        conditionNames: t.conditionsCjs
      });
    return [a, n];
  } catch (e) {
    console.warn("load native resolver failed: ", e);
  }
  return [undefined, undefined];
}
function setImportee2filePath(e, t) {
  undefined !== e?.id && importee2filePath.set(t, e.id);
}
function backSlashExternalSaved(e, t, r, s) {
  return !!isBackslashExternal(e, t, r) && (setImportee2filePath(s, r), true);
}
export function nodeResolve(e = {}) {
  const {
      warnings: t
    } = (0, deprecated_options_js_1)(e),
    r = {
      ...defaults,
      ...e
    },
    {
      extensions: s,
      jail: o,
      moduleDirectories: i,
      modulePaths: a,
      ignoreSideEffectsForRoot: n,
      sdkModulesPath: l,
      useNativeResolver: c
    } = r,
    u = initData(r);
  let d, p, f;
  idToPackageInfo = u.idToPackageInfo, checkModuleDirectories(i);
  const h = updateDedupe(u.dedupe, r),
    _ = getResolveOnly(r),
    g = new Map(),
    m = new Map();
  let j = false;
  const v = new Map(),
    E = new Map(),
    S = new Map();
  let C, k;
  const x = {
    order: "post",
    async handler(e, t, l) {
      if (e === constants_js_1.ES6_BROWSER_EMPTY) return e;
      if (/\0/.test(e)) return null;
      const {
        custom: x = {}
      } = l;
      let R;
      if (/\0/.test(t) && (t = undefined), /^lib\S+\.so/.test(e)) return {
        id: getVirtualFileId(e),
        external: j
      };
      if (!r.caseSensitiveCheck && isExternal(m, v, e, t, this.share?.projectConfig)) return R = await getResolveLikeNode(h, u.rootDir, g, u.useBrowserOverrides, _, d, s, u.conditionsCjs, u.conditionsEsm, u.packageInfoCache, u.mainFields, p, i, a, n, u.preferBuiltins, u.idToPackageInfo, u.isPreferBuiltinsSet, o, r, C, k, c)(this, e, t, x), setImportee2filePath(R, e), {
        id: e,
        external: true
      };
      if ("." !== e[0] && f.has(e) && (R = {
        id: f.get(e)
      }), R || (R = await getResolveLikeNode(h, u.rootDir, g, u.useBrowserOverrides, _, d, s, u.conditionsCjs, u.conditionsEsm, u.packageInfoCache, u.mainFields, p, i, a, n, u.preferBuiltins, u.idToPackageInfo, u.isPreferBuiltinsSet, o, r, C, k, c)(this, e, t, x)), r.caseSensitiveCheck && isExternal(m, v, e, t, this.share?.projectConfig)) return setImportee2filePath(R, e), {
        id: e,
        external: true
      };
      const w = backSlashExternalSaved(E, S, e, R);
      return R && (import_check_plugin_js_1.resolveId.call(this, e, R.id, t, r), R = processFinalResolved(this, e, R, j, w)), R;
    }
  };
  return {
    name: "oh-resolve",
    buildStart(e) {
      importee2filePath.clear(), d = e, t.forEach(e => this.warn(e)), {
        preserveSymlinks: p
      } = e;
      const o = this.share?.projectConfig?.projectTopDir || process.cwd();
      this.share.symlinkMap = (0, resolve_ohmodules_js_1.resolveAllOhModules)(o, this.share.getCachedFileInfo), updateRealpathCache(r.useRealpathCache), f = readSystemApi(new Set(l), this.share.getCachedFileInfo), getExternal(m, v, E, S, this.share.projectConfig), j = !this.share.projectConfig.isFaMode, [C, k] = loadNativeResolver(c, u, s, p, a, r.caseSensitiveCheck), harDuplicateFileSet = (0, har_deduplicate_util_js_1.getHarDuplicateFileSet)(this.share.projectConfig);
    },
    generateBundle: clearCache,
    beforeBuildEnd() {
      cache_js_1.caseErrorSet.forEach(e => {
        logger.error(e);
      });
    },
    resolveId: x,
    load: load,
    transform: transform,
    moduleParsed(e) {
      import_check_plugin_js_1.moduleParsed.call(this, e);
    },
    getPackageInfoForId: e => u.idToPackageInfo.get(e),
    cleanUp: () => {
      m.clear(), E.clear(), S.clear(), f?.clear(), clearCache(), C?.clearCache(), harDuplicateFileSet?.clear(), idToPackageInfo?.clear();
    }
  };
}
export function getExternal(e, t, r, s, o) {
  const i = new Set();
  if (o.compileMode === constants_js_1.ESMODULE) {
    const e = (0, arkts_pack_js_1.loadAceBuildJson)(o.aceBuildJson);
    Object.keys(e?.hspNameOhmMap ?? {}).forEach(e => {
      t.set(new RegExp(`^(${e})($|/\\S*)`, "i"), e), s.set(new RegExp(`^(${e})(\\\\\\S*)`, "i"), e);
    });
  }
  if (o.compileBlockPkg) for (const e of o.compileBlockPkg) i.add(e);
  for (const t of i) e.set(new RegExp(`^(${t})($|/\\S*)`, "i"), t), r.set(new RegExp(`^(${t})(\\\\\\S*)`, "i"), t);
}
function updateRealpathCache(e) {
  e ? (0, fs_js_1.enableRealpathCache)() : (0, fs_js_1.disableRealpathCache)();
}
function clearCache() {
  (0, fs_js_1.clearRealpathCache)(), (0, fs_js_1.clearFileCache)(), (0, cache_js_1.clearStepAddressingCache)(), cache_js_1.readCachedFile.clear(), cache_js_1.isFileCached.clear(), cache_js_1.isDirCached.clear();
}
function transform(e, t) {
  import_check_plugin_js_1.transform.call(this, e, t);
}
function load(e) {
  return e === constants_js_1.ES6_BROWSER_EMPTY ? "export default {};" : e.startsWith("\0hvigor_ignore_") || (0, har_deduplicate_util_js_1.isHspDeduplicateRealCompile)(this.share.projectConfig) && e.startsWith("\0") && harDuplicateFileSet && harDuplicateFileSet.has(e.substring(1)) ? "" : null;
}
function withinModule(e, t) {
  const r = e.share.projectConfig.modulePath;
  return t.id.includes(r);
}
function processFinalResolved(e, t, r, s, o) {
  if (o && (0, har_deduplicate_util_js_1.isHspCollectFilesCompile)(e.share.projectConfig)) return {
    id: t,
    external: true
  };
  if (r.id.endsWith(".d.ts") || r.id.endsWith(".d.ets")) return e.share.projectConfig?.customizedOptions?.basePackage && withinModule(e, r) && (e.share.projectConfig.customHarDeclareFilesMap || (e.share.projectConfig.customHarDeclareFilesMap = {}), e.share.projectConfig.customHarDeclareFilesMap[getVirtualFileId(r.id)] = {
    id: r.id
  }), {
    id: getVirtualFileId(r.id),
    external: s
  };
  const i = processHarDeduplicate(e, r.id, s);
  return i || r;
}
export function processHarDeduplicate(e, t, r) {
  if ((0, har_deduplicate_util_js_1.isHspDeduplicateRealCompile)(e.share.projectConfig) && harDuplicateFileSet && harDuplicateFileSet.has(t)) {
    let s = {
      isHarDeduplicate: true
    };
    const o = e.getModuleInfo(t);
    return o && o.meta && (s = Object.assign(o.meta, s)), {
      id: `\0${t}`,
      external: r,
      meta: s
    };
  }
  return null;
}
export { idToPackageInfo };
export default nodeResolve;
