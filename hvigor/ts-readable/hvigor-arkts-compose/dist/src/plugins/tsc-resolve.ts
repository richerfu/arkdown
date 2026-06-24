/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/tsc-resolve.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as path_cache_stat_js_1 from "../util/path-cache-stat";
import * as har_deduplicate_util_js_1 from "./har-deduplicate/har-deduplicate-util";
import import_check_plugin_js_1 from "./import-check-plugin";
import * as node_resolve_1 from "./node-resolve";
const isWindows = "win32" === process.platform;
let tscProgram;
const ignoreVirtualFilePrefix = "\0hvigor_ignore_",
  libraryResolvedCache = new Map();
let shouldDoCaseSensitiveCheck = false;
const caseErrorSet = new Set();
let harDuplicateFileSet;
const logger = hvigor_arkts_base_1.ArktsBaseLogger.getLogger("caseSensitiveCheck");
function getVirtualFileId(e) {
  return "\0hvigor_ignore_" + e.replace(/\\/g, "_").replace(/\//g, "_");
}
function isDeclarationFile(e) {
  return e.endsWith(".d.ts") || e.endsWith(".d.ets");
}
function isSystemApi(e) {
  const t = path_1.basename(e);
  return t.startsWith("@ohos.") || t.startsWith("@hms.") || t.startsWith("@system.") || t.startsWith("@kit.");
}
function toUnixPath(e) {
  return isWindows ? e.split(path_1.sep).join("/") : e;
}
const harExternal = new Map(),
  hspExternal = new Map(),
  harWithBackSlashExternal = new Map(),
  hspWithBackslashExternal = new Map();
export function tscResolve(e) {
  return {
    name: "tscResolve",
    buildStart: function () {
      const t = path_1.join(this.share.projectConfig.etsLoaderPath, "main.js");
      fs_1.existsSync(t) && (tscProgram = require(t)?.globalProgram?.program), shouldDoCaseSensitiveCheck = e?.caseSensitiveCheck || false, (0, node_resolve_1.getExternal)(harExternal, hspExternal, harWithBackSlashExternal, hspWithBackslashExternal, this.share.projectConfig), harDuplicateFileSet = (0, har_deduplicate_util_js_1.getHarDuplicateFileSet)(this.share.projectConfig);
    },
    load: function (e) {
      if (e.startsWith("\0hvigor_ignore_")) return "";
      if ((0, har_deduplicate_util_js_1.isHspDeduplicateRealCompile)(this.share.projectConfig) && e.startsWith("\0") && harDuplicateFileSet && harDuplicateFileSet.has(e.substring(1))) return "";
      if (tscProgram) {
        const t = tscProgram.getSourceFile(e);
        if (t) return t.text;
      }
      return null;
    },
    resolveId: resolveId,
    beforeBuildEnd: function () {
      caseErrorSet.forEach(e => {
        logger.error(e);
      });
    },
    cleanUp: function () {
      harExternal.clear(), hspExternal.clear(), harWithBackSlashExternal.clear(), hspWithBackslashExternal.clear(), libraryResolvedCache.clear(), caseErrorSet.clear(), harDuplicateFileSet?.clear(), tscProgram = null;
    }
  };
}
function normalizePathOnWindows(e) {
  return isWindows && (e = e.replace(/\//g, "\\")), e;
}
async function resolveId(e, t, a) {
  if (!t || !tscProgram) return null;
  const r = tscProgram.getSourceFile(t);
  if (!r || !r.resolvedModules) return null;
  const s = r.resolvedModules.get(e);
  if (!s || !s.resolvedFileName) return null;
  if (/^lib\S+\.so/.test(e)) return shouldDoCaseSensitiveCheck && (await doCaseCheck(e, t, s)), {
    id: getVirtualFileId(e),
    external: !this.share.projectConfig.isFaMode
  };
  if ((0, node_resolve_1.isExternal)(harExternal, hspExternal, e, t, this.share?.projectConfig)) return shouldDoCaseSensitiveCheck && (await doCaseCheck(e, t, s)), {
    id: e,
    external: true
  };
  if ((0, har_deduplicate_util_js_1.isHspCollectFilesCompile)(this.share.projectConfig) && (0, node_resolve_1.isBackslashExternal)(harWithBackSlashExternal, hspWithBackslashExternal, e)) return {
    id: e,
    external: true
  };
  const i = await resolveLikeNode(e, s, !this.share.projectConfig.isFaMode, this.share.getCachedFileInfo);
  if (!i) return null;
  const {
    finalResolved: l,
    isResolved2SystemApi: n
  } = i;
  if (shouldDoCaseSensitiveCheck && !n && (await doCaseCheck(e, t, s)), l.id && !l.isVirtualFile) {
    import_check_plugin_js_1.resolveId.call(this, e, l.id, t, a), l.id = normalizePathOnWindows(l.id);
    const r = (0, node_resolve_1.processHarDeduplicate)(this, l.id, !this.share.projectConfig.isFaMode);
    if (r) return r;
  }
  return l;
}
async function resolveLikeNode(e, t, a, r) {
  let s,
    i = false;
  if (isDeclarationFile(t.resolvedFileName)) {
    if (t.packageId) {
      const a = await resolveLibrary(e, t, true, r);
      if (!a) return null;
      s = {
        id: a
      };
    } else if ("." !== e[0] && isSystemApi(t.resolvedFileName)) i = true, s = {
      id: getVirtualFileId(t.resolvedFileName),
      external: a,
      isVirtualFile: true
    };else {
      const e = maybeRealSourceFile(t.resolvedFileName, [".js"], r);
      s = e ? {
        id: e
      } : {
        id: getVirtualFileId(t.resolvedFileName),
        external: a,
        isVirtualFile: true
      };
    }
  } else {
    const a = await resolveLibrary(e, t, false, r);
    if (!a) return null;
    s = {
      id: a
    };
  }
  return {
    finalResolved: s,
    isResolved2SystemApi: i
  };
}
function myNormalize(e) {
  return ((e = toUnixPath(e)).endsWith(".js") || e.endsWith(".ts") || e.endsWith(".ets") || e.endsWith(".mjs") || e.endsWith(".cjs")) && (e = e.substring(0, e.length - path_1.extname(e).length)).endsWith(".d") && (e = e.substring(0, e.length - 2)), e;
}
async function doCaseCheck(e, t, a) {
  const r = toUnixPath(fs_1.realpathSync.native(a.resolvedFileName)),
    s = toUnixPath(a.resolvedFileName);
  let i;
  if (undefined !== a.originalPath || r.toLowerCase() !== s.toLowerCase()) {
    let t = myNormalize(path_1.normalize(e));
    t.startsWith("../") && (t = t.replace(/\.+\//g, "")), t.endsWith("/") && (t = t.substring(0, t.length - 1)), i = await checkPathCaseSensitivity(t, myNormalize(a.originalPath || a.resolvedFileName));
  } else i = r === s;
  if (!i) {
    const a = `ERROR File: ${t} \n        Cannot resolve import statement ${e}. Please check the case.`;
    caseErrorSet.add(a);
  }
  return i;
}
async function checkCaseByReadDirFiles(e, t) {
  const a = await fs_1.promises.readdir(e);
  let r = false;
  for (const e of a) {
    if (e === t) {
      r = true;
      break;
    }
    if (path_1.extname(e)) {
      const a = path_1.basename(e, path_1.extname(e));
      if (a === t || a === `${t}.d`) {
        r = true;
        break;
      }
    }
  }
  return r;
}
async function checkPathCaseSensitivity(e, t) {
  const a = e.split("/"),
    r = t.split("/");
  let s = a.length - 1,
    i = r.length - 1;
  for (; s >= 0;) {
    for (; i >= 0 && a[s].toLowerCase() !== r[i].toLowerCase();) i--;
    if (i < 0) return false;
    const e = r.slice(0, i + 1).join("/"),
      t = path_1.dirname(e),
      l = a[s];
    if (!(await checkCaseByReadDirFiles(t, l))) return false;
    s--, i--;
  }
  return true;
}
async function readJson5(e) {
  try {
    const t = await fs_1.promises.readFile(e, "utf-8");
    return (0, hvigor_common_1.parseJsonText)(t);
  } catch (e) {
    return;
  }
}
function getCacheKey(e) {
  return `${e.resolvedFileName}|${e.packageId?.name}|${e.packageId?.version}`;
}
async function resolveLibrary(e, t, a, r) {
  if (!t.packageId) return null;
  const s = libraryResolvedCache.get(getCacheKey(t));
  if (s) return s;
  if (e === t.packageId.name || new RegExp(`^${t.packageId.name}\\/+$`).test(e)) {
    const e = t.resolvedFileName.substring(0, t.resolvedFileName.length - t.packageId.subModuleName.length),
      a = path_1.resolve(e, "oh-package.json5");
    if (path_cache_stat_js_1.PathCacheStat.exists(a, r)) {
      const r = await readJson5(a);
      if (!r) return null;
      let s;
      if (r.module ? s = path_1.resolve(e, r.module) : r.main && (s = path_1.resolve(e, r.main)), s) return libraryResolvedCache.set(getCacheKey(t), s), s;
    }
  } else {
    if (!a) return t.resolvedFileName;
    {
      const e = maybeRealSourceFile(t.resolvedFileName, [".mjs", ".js"], r);
      if (e) return libraryResolvedCache.set(getCacheKey(t), e), e;
    }
  }
  return null;
}
function maybeRealSourceFile(e, t, a) {
  const r = e.endsWith(".d.ets") ? 6 : 5;
  for (const s of t) {
    const t = e.substring(0, e.length - r) + s;
    if (path_cache_stat_js_1.PathCacheStat.exists(t, a)) return t;
  }
  return null;
}
