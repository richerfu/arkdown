/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/arkts-pack.js
 */
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import plugin_commonjs_1 from "@rollup/plugin-commonjs";
import plugin_json_1 from "@rollup/plugin-json";
import plugin_node_resolve_1 from "@rollup/plugin-node-resolve";
import index_js_1 from "../src/plugins/node-resolve/index";
import * as compile_event_factory_js_1 from "./analyzer/compile-event-factory";
import * as compile_perf_js_1 from "./analyzer/compile-perf";
import * as compose_error_js_1 from "./data/compose-error";
import * as check_duplicate_filenames_plugin_js_1 from "./plugins/check-duplicate-filenames-plugin";
import * as collect_importers_plugin_js_1 from "./plugins/collect-importers-plugin";
import * as common_plugin_js_1 from "./plugins/common-plugin";
import copy_plugin_js_1 from "./plugins/copy-plugin";
import delete_plugin_js_1 from "./plugins/delete-plugin";
import * as gc_plugin_js_1 from "./plugins/gc-plugin";
import * as hsp_collect_har_files_js_1 from "./plugins/har-deduplicate/hsp-collect-har-files";
import * as invalid_cache_plugin_js_1 from "./plugins/invalid-cache-plugin";
import * as module_meta_info_plugin_js_1 from "./plugins/module-meta-info-plugin";
import * as preview_resource_watcher_plugin_js_1 from "./plugins/preview-resource-watcher-plugin";
import * as router_map_plugin_js_1 from "./plugins/router-map-plugin";
import * as tsc_resolve_js_1 from "./plugins/tsc-resolve";
import * as use_new_ohm_url_plugin_js_1 from "./plugins/use-new-ohm-url-plugin";
import * as use_sensitive_check_plugin_js_1 from "./plugins/use-sensitive-check-plugin";
import * as wait_async_plugin_js_1 from "./plugins/wait-async-plugin";
import * as wrapper_chunk_plugin_js_1 from "./plugins/wrapper-chunk-plugin";
import * as constants_js_1 from "./util/constants";
import * as copy_resources_js_1 from "./util/copy-resources";
import * as load_sdk_functions_js_1 from "./util/load-sdk-functions";
import * as rust_support_platform_util_js_1 from "./util/rust-support-platform-util";
import * as utils_js_1 from "./util/utils";
import * as compile_code_snippet_js_1 from "./work-scripts/compile-code-snippet";
import * as compile_code_snippet_js_2 from "./work-scripts/compile-code-snippet";
const PROCESS_UN_HANDLED_REJECTION = "unhandledRejection",
  PROCESS_UN_CAUGHT_EXCEPTION = "uncaughtException",
  logger = hvigor_arkts_base_1.ArktsBaseLogger.getLogger("arkts-pack"),
  defaultProjectConfig = {
    watchMode: "false",
    isPreview: false,
    buildMode: constants_js_1.RELEASE,
    compileMode: constants_js_1.JSBUNDLE
  },
  getMergedConfig = e => {
    const t = {
        ...defaultProjectConfig
      },
      s = new Set([...Object.keys(defaultProjectConfig), ...Object.keys(e)]);
    for (const r of s) undefined !== e[r] && (t[r] = e[r]);
    return t.arkTSVersion = e.arkTSVersion, t;
  };
let watcher,
  isWatchFirst = true;
const setWatcher = e => {
  watcher = e;
};
function registerCatchUnhandleError(e, t, s) {
  process.on("unhandledRejection", e => {
    throw e;
  }), process.on("uncaughtException", r => {
    if ("true" === e.watchMode) return watcher?.resolveDonePromise(), s?.error(r), void t?.postMessage(isWatchFirst ? {
      event: hvigor_arkts_base_1.WatchEvent.WATCH_WORK_ERROR
    } : {
      type: hvigor_arkts_base_1.WatchEvent.WATCH_RESULT,
      content: false
    });
    if (r.name === compose_error_js_1.ARKTS_COMPILER_ERROR_NAME) {
      const e = r,
        t = (0, hvigor_arkts_base_1.addCompilerLogMsg)(r.message),
        s = new compose_error_js_1.ExtendedErrorInfo(t.errorMsg, {
          infoMsg: t.infoMsg,
          warnMsg: t.warnMsg
        });
      throw e.stack && s.stack && (s.stack = s.stack.concat(os_1.EOL).concat(e.stack)), s;
    }
    throw r;
  });
}
function getRollupErrorMessage(e) {
  const t = "00305015 Rollup Error\nError Message: ";
  if (e.loc && e.loc.file) return `${t}${e.message + os_1.EOL}. At file: ${e.loc.file}:${e.loc.line}`;
  const s = e.id ? `At file: ${e.id}` : "";
  return `${t}${e.message}. ${s}`;
}
function noop() {}
export async function runArkPack(e, t, s, r, o) {
  e.startEvent = e => compile_event_factory_js_1.CompileEventFactory.createEvent(e, 0, 0).start(), e.endEvent = e => compile_event_factory_js_1.CompileEventFactory.getEventById(e)?.stop();
  const n = new compile_perf_js_1.CompilePerf(e.perf).clear();
  let i;
  hvigor_arkts_base_1.ArktsBaseLogger.clearLogEvents(), isWatchFirst = true, registerCatchUnhandleError(e, r, t);
  let a = noop;
  try {
    a = await collectCompilePerf(e, t, n, s, r, o);
  } catch (e) {
    if ("watchCompileMsg" === e.name) i = e.warnMsg ? new compose_error_js_1.ExtendedErrorInfo(e.errorMsg, {
      infoMsg: e.infoMsg,
      warnMsg: e.warnMsg
    }) : new Error(e.errorMsg);else if (e instanceof Error) {
      const t = "RollupError" === e.name ? getRollupErrorMessage(e) : e.message;
      i = new compose_error_js_1.ArkTsCompilerError(t, e.stack);
    } else i = "string" == typeof e ? new Error(e) : e;
  }
  if (isWatchFirst = false, hvigor_arkts_base_1.ArktsBaseLogger.checkLoggerHaveErrorInfo() && "true" !== e.watchMode && !i && (i = new compose_error_js_1.ArkTsCompilerError("ArkTS Compiler Error")), "true" !== e.watchMode && clearWorkerProcessContext(), i) {
    if (i.name === compose_error_js_1.ARKTS_COMPILER_ERROR_NAME) {
      const e = (0, hvigor_arkts_base_1.addCompilerLogMsg)(i.message),
        t = new compose_error_js_1.ExtendedErrorInfo(e.errorMsg, {
          infoMsg: e.infoMsg,
          warnMsg: e.warnMsg
        });
      i.stack && t.stack && (t.stack = t.stack.concat(os_1.EOL).concat(i.stack)), i = t;
    }
    return await a(false), {
      compileLogEvents: [],
      compileEvents: [],
      isSuccess: false,
      error: i
    };
  }
  return await a(true), {
    compileLogEvents: hvigor_arkts_base_1.ArktsBaseLogger.getLogEvents(),
    compileEvents: updateEventNames(Array.from(n.transform())),
    isSuccess: true
  };
}
async function collectCompilePerf(e, t, s, r, o, n) {
  hvigor_arkts_base_1.projectConfigPath.init(e.projectTopDir, e.sdkPath, e.buildDir);
  const i = e.startEvent("generate configuration information"),
    a = generateConfig(e, t, n);
  if (validComposedOhPackagePathMap(e), checkRustResolver(e, t), e.endEvent(i.getId()), a.input && Object.keys(a.input).length > 0) {
    const i = () => e.isPreview && o && (0, compile_code_snippet_js_1.initServer)(e.cachePath, e.aceModuleBuild, e.etsLoaderPath, o),
      l = await (0, hvigor_arkts_base_1.runHvigorCompile)(a, t, r, o, i, setWatcher, n);
    return collectTimers(l.timing, s), l.callback ?? noop;
  }
  return logger.debug(`Empty input received at runArkPack. BuildStart and corresponding clean will not be executed.\n     Current module path: ${e.projectPath}.`), noop;
}
function checkRustResolver(e, t) {
  e.useNativeResolver && !(0, rust_support_platform_util_js_1.isSupportedPlatform)() && t?.warn("ohos.nativeResolver not supported on the current operating system.");
}
function collectTimers(e, t) {
  if (e) for (const [s, [r, o, n, i, a, l, u]] of Object.entries(e)) t.collect({
    name: s,
    time: r,
    startTime: o,
    endTime: n,
    frequency: i,
    memory: a,
    totalMemory: u,
    startMemory: l
  });
}
function clearProcessListeners() {
  process.removeAllListeners("uncaughtException"), process.removeAllListeners("unhandledRejection");
}
function clearWorkerProcessContext() {
  (0, load_sdk_functions_js_1.clearSdkContext)(), clearProcessListeners();
}
export function getResolveModules(e) {
  const t = e.packageManagerType === constants_js_1.OHPM ? constants_js_1.OH_NODE_MODULES : constants_js_1.NODE_MODULES,
    s = path_1.resolve(e.projectTopDir),
    r = path_1.resolve(e.modulePath),
    o = e.buildGeneratedProfilePath;
  return e.resolveModulePaths = [path_1.resolve(r, "src/main"), o, path_1.resolve(r, t), path_1.resolve(s, t)], e.resolveModulePaths;
}
export function loadAceBuildJson(e) {
  if (e && fs_1.existsSync(e)) try {
    return JSON.parse(fs_1.readFileSync(e).toString());
  } catch (e) {
    logger.error(`parse loader.json failed: ${e.message}`);
  }
  return {};
}
function getPlugins(e, t, s) {
  const r = [...getBuiltInPlugins(e, s)],
    o = loadCustomCompilePlugins(e.hvigorPluginFile, t);
  e.compilePluginPath && o.push(...loadCustomCompilePlugins(e.compilePluginPath, t)), o.forEach(s => {
    const o = s.order ?? "last";
    if ("first" === o && r.unshift(doWrapper(s, e)), "last" === o && r.push(doWrapper(s, e)), "function" != typeof o) return;
    const n = {};
    r.forEach((e, t) => {
      let s = e.name,
        r = 1;
      for (; n[s];) s = `${s}_${r}`, r++;
      n[s] = t;
    });
    const i = o(n);
    ("number" != typeof i || Number.isNaN(i)) && t?.printErrorExit("INVALID_PLUGIN_EXPORT", [e.hvigorPluginFile, s.name, i], [[e.hvigorPluginFile]]), r.splice(i < 0 ? 0 : i, 0, doWrapper(s, e));
  }), e.useTscResolve && r.push((0, tsc_resolve_js_1.tscResolve)({
    caseSensitiveCheck: e.caseSensitiveCheck
  }));
  const n = checkPluginDuplicateName(r);
  return n && t?.printErrorExit("DUPLICATED_PLUGIN_NAME_IN_ROLLUP", [e.compilePluginPath, n], [[e.compilePluginPath, n]]), r;
}
function checkPluginDuplicateName(e) {
  const t = e.map(e => e.name),
    s = new Set();
  for (const e of t) {
    if (s.has(e)) return e;
    s.add(e);
  }
}
function getSupportExtensions() {
  return [...new Set(constants_js_1.SUPPORT_EXTENSIONS.concat(load_sdk_functions_js_1.sdkSupportExtensions))];
}
function getBuiltInPlugins(e, t) {
  if (e.isHarDeduplicateArkCompile) return getHarDeduplicateBuiltInPlugins(e, t);
  const s = getSupportExtensions();
  return [e.isPreview && e.appResource && (0, preview_resource_watcher_plugin_js_1.previewResourceWatcherPlugin)(generatePreviewResourceFiles(e)), (e.isFaMode || "true" === e.watchMode) && (0, copy_plugin_js_1)({
    targets: (0, copy_resources_js_1.getCopyPluginConfig)(e, e.appResource)
  }), (0, common_plugin_js_1.commonPlugin)(e, t), (0, invalid_cache_plugin_js_1.invalidCachePlugin)(), (0, check_duplicate_filenames_plugin_js_1.checkDuplicatedFileNamesPlugin)(), e.compileMode === constants_js_1.ESMODULE && (0, module_meta_info_plugin_js_1.moduleMetaInfoPlugin)(s), getResolvePlugin(e, s), (0, router_map_plugin_js_1.routerMapPlugin)(), (0, use_new_ohm_url_plugin_js_1.newOhmUrlPlugin)(), (0, use_sensitive_check_plugin_js_1.sensitiveCheckPlugin)(), (0, plugin_commonjs_1)(), (0, gc_plugin_js_1.gcPlugin)(), (0, plugin_json_1)(), (0, wrapper_chunk_plugin_js_1.wrapperChunkPlugin)(), (0, wait_async_plugin_js_1.waitAsyncPlugin)(), (0, collect_importers_plugin_js_1.collectImportersPlugin)(e), ...load_sdk_functions_js_1.sdkPlugins, !e.isPreview && (0, delete_plugin_js_1)({
    targets: load_sdk_functions_js_1.cleanConfig,
    runOnce: true
  })].filter(e => "object" == typeof e);
}
function getHarDeduplicateBuiltInPlugins(e, t) {
  const s = getSupportExtensions(),
    r = [];
  return load_sdk_functions_js_1.sdkPlugins.forEach(e => {
    "object" == typeof e && null !== e && ("etsChecker" !== e.name && "etsTransform" !== e.name && "apiTransform" !== e.name && "createProgramPlugin" !== e.name || r.push(e), "genAbc" === e.name && r.push({
      name: "myGenAbc",
      cleanUp: e.cleanUp
    }));
  }), [(0, common_plugin_js_1.commonPlugin)(e, t), (0, invalid_cache_plugin_js_1.invalidCachePlugin)(), (0, check_duplicate_filenames_plugin_js_1.checkDuplicatedFileNamesPlugin)(), e.compileMode === constants_js_1.ESMODULE && (0, module_meta_info_plugin_js_1.moduleMetaInfoPlugin)(s), getResolvePlugin(e, s), (0, router_map_plugin_js_1.routerMapPlugin)(), (0, use_new_ohm_url_plugin_js_1.newOhmUrlPlugin)(), (0, use_sensitive_check_plugin_js_1.sensitiveCheckPlugin)(), (0, plugin_commonjs_1)(), (0, gc_plugin_js_1.gcPlugin)(), (0, plugin_json_1)(), (0, wait_async_plugin_js_1.waitAsyncPlugin)(), (0, collect_importers_plugin_js_1.collectImportersPlugin)(e), (0, hsp_collect_har_files_js_1.hspCollectHarFilesPlugin)(), ...r, !e.isPreview && (0, delete_plugin_js_1)({
    targets: load_sdk_functions_js_1.cleanConfig,
    runOnce: true
  })].filter(e => "object" == typeof e);
}
function getResolvePlugin(e, t) {
  const s = getResolveModules(e),
    r = e.globalModulePaths ?? [];
  return r.push(path_1.resolve(e.etsLoaderPath, "../../api")), e.packageManagerType === constants_js_1.OHPM ? (0, index_js_1)({
    extensions: t,
    modulePaths: Array.from(new Set([path_1.resolve(constants_js_1.OH_NODE_MODULES), ...s])),
    useRealpathCache: true,
    preferBuiltins: false,
    pkgNameToPkgInfo: e.pkgNameToPkgBriefInfo,
    caseSensitiveCheck: e.caseSensitiveCheck,
    useNativeResolver: e.useNativeResolver && (0, rust_support_platform_util_js_1.isSupportedPlatform)(),
    sdkModulesPath: r
  }) : (0, plugin_node_resolve_1)({
    extensions: t,
    modulePaths: Array.from(new Set([path_1.resolve(constants_js_1.NODE_MODULES), path_1.resolve(e.etsLoaderPath, constants_js_1.NODE_MODULES), ...s])),
    preferBuiltins: false
  });
}
function generatePreviewResourceFiles(e) {
  return [path_1.resolve(__dirname, e.appResource), (0, utils_js_1.getOhPackageJsonPath)(path_1.resolve(e.modulePath, e.packageManagerType === constants_js_1.OHPM ? constants_js_1.OH_PACKAGE_JSON5 : constants_js_1.PACKAGE_JSON))];
}
function loadCustomCompilePlugins(e, t) {
  const s = [];
  if (!e) return s;
  let r;
  delete require.cache[require.resolve(e)];
  try {
    r = require(e).default;
  } catch (t) {
    throw t.message = `${t.message} ${e}`, t;
  }
  r || t?.printErrorExit("PLUGIN_FILE_NO_DEFAULT_EXPORT", [e], [[e]]);
  return (Array.isArray(r) ? r : [r]).forEach(r => {
    validatePlugin(r, e, t), r || t?.printErrorExit("EXPORT_UNDEFINED_PLUGIN", [e], [[e]]), s.push(r);
  }), s;
}
function validatePlugin(e, t, s) {
  "object" != typeof e && s?.printErrorExit("INVALID_PLUGIN_TYPE", [t, typeof e], [[t]]);
  const r = e.name;
  r || s?.printErrorExit("PLUGIN_NAME_NOT_EXIST", [t], [[t]]), "string" != typeof r && s?.printErrorExit("PLUGIN_NAME_NOT_STRING", [t], [[t]]), Object.keys(e).forEach(o => {
    const n = e[o];
    switch (o) {
      case "name":
        break;
      case "order":
        n && "last" !== n && "first" !== n && "function" != typeof n && s?.printErrorExit("PLUGIN_NAME_WITH_UNKNOWN_ORDER", [t, r, n], [[r, t]]);
        break;
      case "buildStart":
      case "resolveId":
      case "load":
      case "transform":
      case "shouldInvalidCache":
      case "moduleParsed":
      case "beforeBuildEnd":
      case "buildEnd":
        n && "function" != typeof n && s?.printErrorExit("PLUGIN_NAME_WITH_UNKNOWN_HOOK", [t, r, o], [[r, o, t]]);
        break;
      default:
        s?.warn(`'${t}' exports an invalid plugin '${r}' with unknown property: '${o}'.`);
    }
  });
}
function manualChunks(e, {
  getModuleInfo: t
}) {
  if (e.includes(constants_js_1.NODE_MODULES) || e.includes(constants_js_1.OH_NODE_MODULES)) return "commons";
  const s = t(e);
  return s && !s.isEntry ? "vendors" : undefined;
}
function getCompileInputObject(e) {
  const t = {};
  return Object.entries(e).forEach(([e, s]) => {
    if (!s.endsWith(".d.ts") && !s.endsWith(".d.ets")) {
      const r = e.startsWith("../") ? `relative/../${e}` : e;
      t[r] = s;
    }
  }), t;
}
function processMockConfigModuleInfos(e) {
  const t = e.mockParams?.mockConfigKey2ModuleInfo;
  if (!t || !Object.keys(t).length) return;
  const s = getSupportExtensions();
  Object.keys(t).forEach(e => {
    const r = t[e],
      o = (0, hvigor_arkts_base_1.findSuitableFilePath)(r.filePath, s);
    o ? r.filePath = o : delete t[e];
  });
}
function getCompileEnv(e, t) {
  return [e.compileSdkVersion, e.compatibleSdkVersion, e.runtimeOS, e.etsLoaderPath, !!e.obfuscationOptions?.selfConfig?.ruleOptions?.enable, t.moduleType, path_1.resolve(t.cachePath, hvigor_arkts_base_1.MSGPACK_COMPILE_CACHE_DIR_NAME)].join("#");
}
function generateConfig(e, t, s) {
  const r = getMergedConfig(e);
  (0, load_sdk_functions_js_1.loadSdkFunctions)(r);
  const o = r.compileMode?.toLowerCase(),
    n = r.buildMode?.toLowerCase(),
    i = r.cachePath,
    a = n === constants_js_1.DEBUG ? path_1.resolve(__dirname, r.aceModuleBuild) : path_1.join(i, "releaseAssets", path_1.basename(r.aceModuleBuild)),
    l = getCompileEnv(e, r),
    u = s?.mount("KeyManager"),
    c = u?.getCache(`${e.modulePath}:${e.targetName}`);
  return c && c !== l && s?.unmount(c), u?.setCache(`${e.modulePath}:${e.targetName}`, l, true), r.compileEnv = l, processMockConfigModuleInfos(r), {
    perf: e.perf === constants_js_1.AnalyzeMode.VERBOSE || e.perf === constants_js_1.AnalyzeMode.TRACE,
    unsupportedChangedFiles: [(0, utils_js_1.getOhPackageJsonPath)(path_1.resolve(e.modulePath, e.packageManagerType === constants_js_1.OHPM ? constants_js_1.OH_PACKAGE_JSON5 : constants_js_1.PACKAGE_JSON))],
    generateModuleGraphOnly: o === constants_js_1.ESMODULE,
    input: getCompileInputObject("true" === e.widgetCompile ? load_sdk_functions_js_1.widgetEntryObj : load_sdk_functions_js_1.entryObj),
    output: getOutputConfig(a, o, e),
    shimMissingExports: true,
    preserveSymlinks: false,
    context: "globalThis",
    treeshake: n === constants_js_1.RELEASE,
    disableGenerateBundle: o === constants_js_1.ESMODULE,
    plugins: getPlugins(r, t, s),
    watch: getWatchConfig(o),
    watchMode: e.watchMode,
    enableFileSystemCache: true,
    cacheDirectory: i,
    onwarn: function (e) {
      onRollupWarn(e.message, e.code, r.isFaMode);
    },
    startEvent: e.startEvent,
    endEvent: e.endEvent,
    isPreview: e.isPreview,
    skipBuildEnd: e.skipBuildEnd,
    compileEnv: l,
    pkgName2SourceRoots: (0, utils_js_1.getPkgName2SourceRootsMap)(e),
    extensions: getSupportExtensions(),
    depName2RootPath: e.depName2RootPath,
    modulePath: e.modulePath,
    isByteCodeHar: e.byteCodeHar,
    declaredFilesPath: e.declaredFilesPath,
    useAcornLoose: !e.shouldEmitJs,
    etsLoaderPath: e.etsLoaderPath,
    bundledDependencies: e.projectArkOption?.bundledDependencies,
    shouldCollectDirectImportees: e.shouldCollectDirectImportees,
    useSourceHash: e.useSourceHash,
    usePathPlaceholder: e.usePathPlaceholder,
    hotReloadBuild: e.hotReloadBuild,
    arkCompileCachePath: e.arkCompileCachePath,
    buildMode: e.buildMode,
    writeRollupCache: e.writeRollupCache
  };
}
function getOutputConfig(e, t, s) {
  return [{
    dir: e,
    chunkFileNames: "[name].js",
    format: "cjs",
    sourcemap: t === constants_js_1.JSBUNDLE && !s.isPreview,
    sourcemapExcludeSources: true,
    sourcemapPathTransform: (e, t) => {
      if (s.buildMode === constants_js_1.RELEASE) {
        const r = loadAceBuildJson(s.aceBuildJson);
        return path_1.resolve(path_1.dirname(t), e).replace(r.projectRootPath, "");
      }
      return path_1.resolve(path_1.dirname(t), e).replace(s.projectTopDir + path_1.sep, "");
    },
    exports: "named",
    sanitizeFileName: false,
    bundleMode: t === constants_js_1.JSBUNDLE,
    manualChunks: s.supportChunks ? manualChunks : undefined
  }];
}
function getWatchConfig(e) {
  return {
    chokidar: {
      useFsEvents: false
    },
    skipWrite: e === constants_js_1.ESMODULE,
    buildDelay: 100
  };
}
export function watch(e, t, s, r) {
  return e.watchMode = "true", runArkPack(e, t, s, r);
}
function wrapperBeforeBuildEnd(e, t) {
  return function (s) {
    const r = getCompilePluginContext(this);
    let o;
    try {
      o = e.beforeBuildEnd.apply(r, [s]);
    } catch (e) {
      throw e.message = `${e.message} ${t.hvigorPluginFile}`, e;
    }
    return o;
  };
}
function wrapperBuildEnd(e, t) {
  return function (s) {
    const r = getCompilePluginContext(this);
    try {
      e.buildEnd.apply(r, [s]);
    } catch (e) {
      throw e.message = `${e.message} ${t.hvigorPluginFile}`, e;
    }
  };
}
function wrapperModuleParsed(e, t) {
  return function (s) {
    const r = getCompilePluginContext(this);
    try {
      e.moduleParsed.apply(r, [s]);
    } catch (e) {
      throw e.message = `${e.message} ${t.hvigorPluginFile}`, e;
    }
  };
}
function wrapperShouldInvalidCache(e, t) {
  return function (s) {
    const r = getCompilePluginContext(this);
    let o;
    try {
      o = e.shouldInvalidCache.apply(r, [s]);
    } catch (e) {
      throw e.message = `${e.message} ${t.hvigorPluginFile}`, e;
    }
    return o;
  };
}
function wrapperTransform(e, t) {
  return function (s, r) {
    const o = getCompilePluginContext(this);
    let n;
    try {
      n = e.transform.apply(o, [s, r]);
    } catch (e) {
      throw e.message = `${e.message} ${t.hvigorPluginFile}`, e;
    }
    return n;
  };
}
function wrapperLoad(e, t) {
  return function (s) {
    const r = getCompilePluginContext(this);
    let o;
    try {
      o = e.load.apply(r, [s]);
    } catch (e) {
      throw e.message = `${e.message} ${t.hvigorPluginFile}`, e;
    }
    return o;
  };
}
function wrapperResolveId(e, t) {
  return function (s, r, o) {
    const n = getCompilePluginContext(this);
    let i;
    try {
      i = e.resolveId.apply(n, [s, r, o]);
    } catch (e) {
      throw e.message = `${e.message} ${t.hvigorPluginFile}`, e;
    }
    return i;
  };
}
function validComposedOhPackagePathMap(e) {
  e.ohPackagePathMap && e.ohPackagePathMap.forEach((e, t) => {
    utils_js_1.composeOhPackagePathMap.set(t, e);
  });
}
function wrapperBuildStart(e, t) {
  return function (s) {
    const r = getCompilePluginContext(this);
    try {
      e.buildStart.apply(r);
    } catch (e) {
      throw e.message = `${e.message} ${t.hvigorPluginFile}`, e;
    }
  };
}
function doWrapper(e, t) {
  const s = {
    name: e.name
  };
  return e.buildStart && (s.buildStart = wrapperBuildStart(e, t)), e.resolveId && (s.resolveId = wrapperResolveId(e, t)), e.load && (s.load = wrapperLoad(e, t)), e.transform && (s.transform = wrapperTransform(e, t)), e.shouldInvalidCache && (s.shouldInvalidCache = wrapperShouldInvalidCache(e, t)), e.moduleParsed && (s.moduleParsed = wrapperModuleParsed(e, t)), e.beforeBuildEnd && (s.beforeBuildEnd = wrapperBeforeBuildEnd(e, t)), e.buildEnd && (s.buildEnd = wrapperBuildEnd(e, t)), s;
}
function getCompilePluginContext(e) {
  return {
    getFileName: t => e.getFileName(t),
    getModuleIds: () => e.getModuleIds(),
    getModuleInfo: t => e.getModuleInfo(t),
    parse: (t, s) => e.parse(t, s),
    share: e.share
  };
}
function updateEventNames(e) {
  const [t, s] = initUpdateEventNameHandler(),
    r = ["beforeBuildEnd", "buildEnd", "afterBuildEnd", "beforeBuild", "buildStart", "afterBuildStart", "resolveId", "load", "resolveDynamicImport", "shouldTransformCachedModule", "transform", "moduleParsed"];
  return e.forEach(e => {
    const o = e.getName(),
      n = o.lastIndexOf("-");
    if (t.has(o)) t.get(o)(e);else for (const t of r) if (o.endsWith(t)) {
      const r = s.get(t);
      r && r(e, o, n);
      break;
    }
  }), e;
}
function onRollupWarn(e, t, s = false) {
  "SHIMMED_EXPORT" === t || s && "SOURCEMAP_BROKEN" === t || console.warn("[33m", e, "[39m");
}
function getCallback(e) {
  return t => t.setName(e);
}
function initUpdateEventNameHandler() {
  const e = new Map();
  e.set("# BUILD", getCallback("generate build graph for source files")), e.set("## initialize", getCallback("initialize compiler input")), e.set("## generate module graph", getCallback("generate module graph")), e.set("## sort and bind modules", getCallback("sort and bind modules")), e.set("## mark included statements", getCallback("mark included statements")), e.set("## before build end", getCallback("before build end")), e.set("## build end", getCallback("build end")), e.set("## after build end", getCallback("after build end"));
  const t = new Map();
  return t.set("beforeBuildEnd", (e, t, s) => e.setName(`${t.substring(0, s)}- execute hook function before build ends`)), t.set("buildEnd", (e, t, s) => e.setName(`${t.substring(0, s)}- execute hook function when build ends`)), t.set("afterBuildEnd", (e, t, s) => e.setName(`${t.substring(0, s)}- execute hook function after build ends`)), t.set("beforeBuild", (e, t, s) => e.setName(`${t.substring(0, s)}- execute hook function before build starts`)), t.set("buildStart", (e, t, s) => e.setName(`${t.substring(0, s)}- execute hook function when build starts`)), t.set("afterBuildStart", (e, t, s) => e.setName(`${t.substring(0, s)}- execute hook function after build starts`)), t.set("resolveId", (e, t, s) => e.setName(`${t.substring(0, s)}- resolve file ID`)), t.set("load", (e, t, s) => e.setName(`${t.substring(0, s)}- load file content`)), t.set("resolveDynamicImport", (e, t, s) => e.setName(`${t.substring(0, s)}- resolve dynamic import`)), t.set("shouldTransformCachedModule", (e, t, s) => e.setName(`${t.substring(0, s)}- decide whether to transform cached module`)), t.set("transform", (e, t, s) => e.setName(`${t.substring(0, s)}- transform file content`)), t.set("moduleParsed", (e, t, s) => e.setName(`${t.substring(0, s)}- execute hook function when module parsed`)), [e, t];
}
export { compileCodeSnippet, compileCodeSnippet };
