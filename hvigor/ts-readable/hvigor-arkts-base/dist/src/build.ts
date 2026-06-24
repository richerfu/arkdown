/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build.js
 */
import * as rollup_1 from "rollup";
import * as clean_ets_for_tgz_js_1 from "./build-steps/clean-ets-for-tgz";
import * as process_importee_js_1 from "./build-steps/process-importee";
import * as rm_declared_files_js_1 from "./build-steps/rm-declared-files";
import * as rm_error_files_cache_js_1 from "./build-steps/rm-error-files-cache";
import * as watch_js_1 from "./build-steps/watch";
import * as rollup_cache_service_js_1 from "./cache/rollup/rollup-cache-service";
import * as arkts_base_log_js_1 from "./log/arkts-base-log";
import * as constants_js_1 from "./util/constants";
import * as project_config_path_js_1 from "./util/project-config-path";
import path_1 from "path";
import fs_extra_1 from "fs-extra";
export async function build(e, r, t, o, a, c, s) {
  (0, clean_ets_for_tgz_js_1.cleanEtsForTgz)(e, r), await removePerformanceRecorderJson(), readCacheWithEvent(e, s);
  const i = (0, rm_declared_files_js_1.getModuleLastCachedSourceFiles)(e);
  rollup_cache_service_js_1.rollupCacheService.removeUncacheableModules(e);
  const l = e.compileEnv,
    n = e.writeRollupCache,
    _ = !!e.disableGenerateBundle,
    d = undefined === e.writeBundle || e.writeBundle,
    u = e.watchMode ?? "false",
    p = e.unsupportedChangedFiles ?? [],
    {
      enableFileSystemCache: h,
      cacheDirectory: g,
      isPreview: f,
      pkgName2SourceRoots: m,
      isByteCodeHar: j,
      modulePath: v,
      declaredFilesPath: P,
      bundledDependencies: w,
      shouldCollectDirectImportees: C
    } = e;
  deleteOptions(e);
  const b = e.startEvent(constants_js_1.ROLLUP_EVENT_NAME);
  if ("true" === u) {
    const s = e => {
        r.error(e);
      },
      i = await (0, watch_js_1.startWatch)(e, f ?? false, t, o, a, c, h, g, p, s);
    return e.endEvent(b.getId()), i;
  }
  const y = await (0, rollup_1.rollup)(e);
  if (e.endEvent(b.getId()), !_) for (const r of e.output) d ? await y.write(r) : await y.generate(r);
  await (0, process_importee_js_1.processImportees)(e, {
    isByteCodeHar: j,
    bundledDependencies: w,
    shouldCollectDirectImportees: C
  }, v, g, y.cache), await (0, rm_declared_files_js_1.removeRedundantDeclaredFiles)(e, i, P, v, y.cache), await (0, rm_error_files_cache_js_1.removeCompileFailedFilesFromCache)(e, g, s, l, y.cache);
  const E = e.startEvent("write build package cache");
  n && (await rollup_cache_service_js_1.rollupCacheService.writeCache({
    enableFileSystemCache: h,
    cache: y.cache,
    cacheDirectory: g,
    pkgName2SourceRoots: m,
    cacheKey: l,
    cacheStoreManager: s,
    parentEvent: E,
    changedModules: y.changedModules,
    usePathPlaceholder: true === e.usePathPlaceholder
  })), e.endEvent(E.getId());
  const R = e.startEvent("wait for plug-in registration asynchronous task to complete");
  return await y.triggerBundleEnd(), e.endEvent(R.getId()), arkts_base_log_js_1.ArktsBaseLogger.reset(), {
    timing: y.getTimings && y.getTimings(),
    callback: async e => {
      await outputPerformanceRecorderJson(y), g && (await rollup_cache_service_js_1.rollupCacheService.onBuildEnd(g, y, l, e, m, s));
    }
  };
}
function deleteOptions(e) {
  ["disableGenerateBundle", "writeBundle", "enableFileSystemCache", "cacheDirectory", "watchMode", "unsupportedChangedFiles", "isPreview", "compileEnv", "extensions", "pkgName2SourceRoots", "resolveConflictMode", "depName2RootPath", "modulePath", "isByteCodeHar", "declaredFilesPath", "bundledDependencies", "shouldCollectDirectImportees", "arkCompileCachePath", "buildMode", "writeRollupCache"].forEach(r => {
    Reflect.deleteProperty(e, r);
  });
}
function readCacheWithEvent(e, r) {
  const t = e.startEvent("read build package cache");
  rollup_cache_service_js_1.rollupCacheService.readCache(e, r), e.endEvent(t.getId());
}
function getPerformanceRecorderPath() {
  if (project_config_path_js_1.projectConfigPath.projectTopPath) return path_1.resolve(project_config_path_js_1.projectConfigPath.projectTopPath, ".hvigor", "outputs", "records", "performance-recorder.json");
}
const _log = arkts_base_log_js_1.ArktsBaseLogger.getHvigorConsoleLogger("recordLog");
async function removePerformanceRecorderJson(e) {
  try {
    const e = getPerformanceRecorderPath();
    e && fs_extra_1.existsSync(e) && (await fs_extra_1.remove(e));
  } catch (e) {
    _log.printDebug(`removePerformanceRecorderJson error, error massage: ${e}`);
  }
}
async function outputPerformanceRecorderJson(e, r) {
  try {
    const r = getPerformanceRecorderPath();
    e.performanceRecords && r && (await fs_extra_1.outputFile(r, JSON.stringify(e.performanceRecords)));
  } catch (e) {
    _log.printDebug(`removePerformanceRecorderJson error, error massage: ${e}`);
  }
}
