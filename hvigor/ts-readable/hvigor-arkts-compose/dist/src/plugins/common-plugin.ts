/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/common-plugin.js
 */
import path_1 from "path";
import * as worker_threads_1 from "worker_threads";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import fs_extra_1 from "fs-extra";
import * as compile_event_factory_js_1 from "../analyzer/compile-event-factory";
import * as compose_error_js_1 from "../data/compose-error";
import * as constants_js_1 from "../util/constants";
import * as utils_js_1 from "../util/utils";
import * as import_resolver_js_1 from "./node-resolve/import-resolver";
const DEP_INFO_CACHE_FILE = "dep_info.json",
  CUSTOM_WORKER_REQUEST = "CUSTOM_WORKER_REQUEST",
  LOGGER = hvigor_arkts_base_1.ArktsBaseLogger.getLogger("commonPlugin");
function addIsSourceOrExternalCodeMethod(e) {
  const o = e.share.projectConfig.sdkPath.toLowerCase();
  e.share.projectConfig.isSourceOrExternalCode = function (e) {
    return -1 === e.indexOf("oh_modules") && !e.startsWith(o);
  };
}
export function commonPlugin(e, o) {
  let r = true,
    t = false;
  return {
    name: "commonPlugin",
    options() {
      this.share.projectConfig = e, addIsSourceOrExternalCodeMethod(this), this.share.getLogger = hvigor_arkts_base_1.ArktsBaseLogger.getPluginLogger, this.share.getHvigorConsoleLogger = hvigor_arkts_base_1.ArktsBaseLogger.getHvigorConsoleLogger, this.share.throwArkTsCompilerError = compose_error_js_1.throwArkTsCompilerError, this.share.getHookEventFactory = e.perf === constants_js_1.AnalyzeMode.VERBOSE || e.perf === constants_js_1.AnalyzeMode.TRACE ? compile_event_factory_js_1.CompileEventFactory.getHookEventFactory : getBogusHookEventFactory, this.share.getSourceFiles = loadSdkGetModuleSourceFilesFunc(e.etsLoaderPath), this.share.importResolver = new import_resolver_js_1.ImportResolver(e.depName2DepInfo, !!e.caseSensitiveCheck, this.share.getCachedFileInfo).resolve, t = !!e.needCoverageInsert, this.share.getHashByFilePath = e => (0, hvigor_arkts_base_1.getHashByFilePath)(e), this.share.setHashValueByFilePath = (e, o) => (0, hvigor_arkts_base_1.setHashValueByFilePath)(e, o), this.share.beforeObfuscate = e.transformAbc ? registerBeforeObfuscate(e.isMainThread ?? false, e.transformAbc) : undefined, (0, utils_js_1.isLinux)() || e.runtimeOS === constants_js_1.OPEN_HARMONY || e.isFaMode || (e.apiVersion >= constants_js_1.API_VERSION_11 && (this.share.cacheStoreManager = o, this.share.cache = e.compileEnv ? o?.mount(e.compileEnv) : undefined), this.share.commonCache = o?.mount(constants_js_1.TSC_COMMON_CACHE_KEY, true), this.share.recordErrorFileId = e => {
        hvigor_arkts_base_1.ArktsBaseLogger.recordErrorFileId(e);
      });
    },
    beforeBuild() {
      (!r || e.isPreview && !fs_extra_1.existsSync(e.buildDir)) && (r = false, (0, compose_error_js_1.throwArkTsCompilerError)("Please restart previewer."));
      const {
        resolveConflictMode: o,
        depName2RootPath: t,
        depName2DepInfo: s
      } = readDepInfoCache(this);
      this.share.depInfo = getDepInfo(e.resolveConflictMode, o, e.depName2RootPath, t, e.depName2DepInfo, s), writeDepInfoCache(e.resolveConflictMode, e.depName2RootPath, e.depName2DepInfo, e.cachePath, this);
    },
    beforeBuildEnd: {
      order: "post",
      async handler() {
        throwErrorIfHasErrorInfo(e.watchMode);
      }
    },
    transform(r, s) {
      s.endsWith("?commonjs-entry") ? this.share.projectConfig.needCoverageInsert = false : this.share.projectConfig.needCoverageInsert = t, hvigor_arkts_base_1.rollupCacheService.deleteModuleCache(s, e.cachePath, o, e.compileEnv);
    }
  };
}
function readDepInfoCache(e) {
  const o = {
      resolveConflictMode: undefined,
      depName2RootPath: undefined,
      depName2DepInfo: undefined
    },
    r = e.cache.get("dep_info.json");
  if (!r) return o;
  let t, s;
  return r?.depName2RootPath && (t = new Map(), Object.keys(r.depName2RootPath).forEach(e => t?.set(e, r.depName2RootPath[e]))), r?.depName2DepInfo && (s = new Map(), Object.keys(r.depName2DepInfo).forEach(e => s?.set(e, r.depName2DepInfo[e]))), {
    resolveConflictMode: r?.resolveConflictMode,
    depName2RootPath: t,
    depName2DepInfo: s
  };
}
function writeDepInfoCache(e, o, r, t, s) {
  const a = {
    resolveConflictMode: e,
    depName2RootPath: o?.size ? {} : undefined,
    depName2DepInfo: r?.size ? {} : undefined
  };
  if (o.forEach((e, o) => a.depName2RootPath[o] = e), r.forEach((e, o) => a.depName2DepInfo[o] = e), s.cache.set("dep_info.json", a), !t) return;
  const n = path_1.resolve(t, "dep_info.json");
  fs_extra_1.ensureFileSync(n), fs_extra_1.writeJSONSync(n, a);
}
function getDepInfo(e, o, r, t, s, a) {
  if (undefined === o) return {
    enableIncre: false,
    changedDeps: []
  };
  if (e && o) {
    const e = [];
    return r.forEach((o, r) => {
      t?.get(r) && t?.get(r) !== o && e.push({
        name: r,
        lastRootPath: t?.get(r)
      });
    }), s.forEach((o, r) => {
      const t = a?.get(r);
      t && o.pkgName === t.pkgName && o.pkgVersion !== t.pkgVersion && e.push({
        name: r,
        lastRootPath: t.pkgRootPath
      });
    }), t?.forEach((o, t) => {
      r.has(t) || e.push({
        name: t,
        lastRootPath: o
      });
    }), {
      enableIncre: true,
      changedDeps: e
    };
  }
  return {
    enableIncre: false,
    changedDeps: []
  };
}
function getBogusHookEventFactory() {
  const e = () => {},
    o = () => ({
      start: e,
      stop: e,
      startAsyncEvent: e,
      stopAsyncEvent: e,
      createSubEvent: o
    });
  return {
    createEvent: o
  };
}
function throwErrorIfHasErrorInfo(e) {
  hvigor_arkts_base_1.ArktsBaseLogger.checkLoggerHaveErrorInfo() && "true" !== e && (0, compose_error_js_1.throwArkTsCompilerError)("ArkTS Compiler Error");
}
function loadSdkGetModuleSourceFilesFunc(e) {
  const o = path_1.join(e, "lib", "fast_build", "ark_compiler", "module", "module_source_file.js");
  if (fs_extra_1.existsSync(o)) try {
    return require(o).ModuleSourceFile.getSourceFiles;
  } catch (e) {
    return void LOGGER.debug(`load getSourceFiles failed ${e}.`);
  } else LOGGER.debug(`${o} not exists.`);
}
function registerBeforeObfuscate(e, o) {
  return (r, t) => {
    if (t.isArkGuardEnabled = !!t.isArkGuardEnabled, !(e && o instanceof Function)) return new Promise(e => {
      const o = r => {
        r.event === CUSTOM_WORKER_REQUEST && r.customType === constants_js_1.TRANSFORM_ABC_WORKER_REQUEST && r.done && (worker_threads_1.parentPort?.off("message", o), e());
      };
      worker_threads_1.parentPort?.on("message", o), worker_threads_1.parentPort?.emit(CUSTOM_WORKER_REQUEST, {
        param: {
          abcPath: r,
          config: t
        },
        customType: constants_js_1.TRANSFORM_ABC_WORKER_REQUEST
      });
    });
    try {
      return o({
        abcPath: r,
        config: t
      });
    } catch (e) {
      return LOGGER.debug(`action of customType: ${constants_js_1.TRANSFORM_ABC_WORKER_REQUEST} throws error: ${e}`), Promise.resolve();
    }
  };
}
