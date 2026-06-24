/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/watch.js
 */
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
import process_1 from "process";
import * as fse from "fs-extra";
import * as rollup_1 from "rollup";
import * as rollup_cache_service_js_1 from "../cache/rollup/rollup-cache-service";
import * as arkts_base_log_js_1 from "../log/arkts-base-log";
import * as ohos_logger_js_1 from "../log/ohos-logger";
import * as build_event_js_1 from "../util/build-event";
import * as constants_js_1 from "../util/constants";
const logger = arkts_base_log_js_1.ArktsBaseLogger.getLogger("build"),
  extensionSet = new Set([".ets", ".ts", "js", ".mjs", ".cjs"]);
let triggerTsWatch,
  watchParentPort,
  watchCache,
  watchEnableFileSystemCache,
  watchCacheDirectory,
  lastWatchRes,
  tsWatchEnd = false,
  rollupWatchEnd = false,
  isWatchFirst = true,
  pausePrintError = false;
const lastReUsedFiles = new Set();
function handleWatchCompileFailed(e) {
  if (isWatchFirst && arkts_base_log_js_1.ArktsBaseLogger.checkLoggerHaveErrorInfo()) {
    e(arkts_base_log_js_1.ArktsBaseLogger.getLogEvents().filter(e => "error" === e.level).map(e => e.msg).join(""));
  }
}
function handleWatchCompileSucceed(e, t) {
  "BUNDLE_END" === e.code && t(e?.result?.getTimings ? {
    timing: e.result.getTimings()
  } : {});
}
function setParentProcessListener(e) {
  watchParentPort.on("message", t => {
    "HOT_COMPILE" === t.event && e.emit("event", {
      code: "HOT_COMPILE"
    }), "PAUSE_PRINT_ERR" === t.event && (pausePrintError = t.data);
  });
}
function handleEventErrorMsg(e, t) {
  if (undefined === e.error?.message && 0 === arkts_base_log_js_1.ArktsBaseLogger.getLogEvents().length) t("Compilation failed");else {
    t({
      ...(0, ohos_logger_js_1.addCompilerLogMsg)(e.error.message ?? "ArkTS Compile Error"),
      name: "watchCompileMsg"
    });
  }
}
export async function startWatch(e, t, a, s, r, o, c, i, l, n) {
  arkts_base_log_js_1.ArktsBaseLogger.addLastLogEvents(readWatchLogs(i)), arkts_base_log_js_1.ArktsBaseLogger.setLogCallback(a, processTsWatchEnd), watchParentPort = s;
  const h = new Set();
  return new Promise((a, s) => {
    const _ = (0, rollup_1.watch)(e);
    let g = false;
    o(_), setParentProcessListener(_), _.on("event", e => {
      switch (e.code) {
        case "BUNDLE_START":
          watchParentPort?.postMessage({
            type: build_event_js_1.WatchEvent.WATCH_START
          }), validateUnsupportedChangedFiles(e.lastChangedFiles, l), updateTriggerTsWatchWithLastChangedFiles(e.lastChangedFiles);
          break;
        case "STOP_HOT_COMPILE":
          break;
        case "BUNDLE_END":
          g = !!e.hotCompile, handleWatchCompileFailed(s), r(), updateCache(c, e.result.cache, i), hotReload(t, e.result.watchFiles, h), handleWatchCompileSucceed(e, a), updateLastReUsedFiles(e.lastReUsedFiles), processRollupWatchEnd(g);
          break;
        case "ERROR":
          handleEventErrorMsg(e, s), arkts_base_log_js_1.ArktsBaseLogger.addLogEvent({
            time: new Date(),
            category: "watch",
            level: "error",
            msg: e.error.message,
            type: constants_js_1.PLUGIN_LOG_TYPE
          }), isWatchFirst || !pausePrintError && n && n(e.error), g = !!e.hotCompile, processRollupWatchEnd(g);
      }
    });
  });
}
export async function writeWatchCache(e) {
  if (lastWatchRes) {
    if (watchCacheDirectory && fs_1.existsSync(watchCacheDirectory)) {
      await rollup_cache_service_js_1.rollupCacheService.writeCache({
        enableFileSystemCache: watchEnableFileSystemCache,
        cache: watchCache,
        cacheDirectory: watchCacheDirectory,
        pkgName2SourceRoots: e,
        useTempFile: false,
        usePathPlaceholder: false
      });
      const t = {
        close: () => Promise.resolve(),
        cache: watchCache
      };
      await rollup_cache_service_js_1.rollupCacheService.onBuildEnd(watchCacheDirectory, t, undefined, lastWatchRes, e, undefined);
    }
    writeWatchLogs();
  } else rollup_cache_service_js_1.rollupCacheService.deleteCache(watchCacheDirectory), removeWatchLogs();
  watchCache = undefined, watchEnableFileSystemCache = undefined, watchCacheDirectory = undefined;
}
function readWatchLogs(e) {
  let t = [];
  const a = path_1.resolve(e ?? "", constants_js_1.DEFAULT_WATCH_LOGS_FILE);
  if (fs_1.existsSync(a)) try {
    t = JSON.parse(fs_1.readFileSync(a).toString());
  } catch (e) {
    logger.debug(`read watch logs failed ${e.message}`);
  }
  return t;
}
function writeWatchLogs() {
  const e = path_1.resolve(watchCacheDirectory ?? "", constants_js_1.DEFAULT_WATCH_LOGS_FILE);
  try {
    fse.ensureFileSync(e), fs_1.writeFileSync(e, JSON.stringify(arkts_base_log_js_1.ArktsBaseLogger.getLastLogEvents()));
  } catch (e) {
    logger.debug(`write watch logs failed ${e.message}`);
  }
}
function removeWatchLogs() {
  const e = path_1.resolve(watchCacheDirectory ?? "", constants_js_1.DEFAULT_WATCH_LOGS_FILE);
  fs_1.rmSync(e, {
    force: true
  });
}
function clearWatchData() {
  triggerTsWatch = false, tsWatchEnd = false, rollupWatchEnd = false, arkts_base_log_js_1.ArktsBaseLogger.clearLogEvents(), arkts_base_log_js_1.ArktsBaseLogger.clearLogMessages(), lastReUsedFiles.clear();
}
function sendWatchResult(e = false) {
  isWatchFirst = false, arkts_base_log_js_1.ArktsBaseLogger.sendLastLogEvents(lastReUsedFiles), lastWatchRes = !arkts_base_log_js_1.ArktsBaseLogger.checkLoggerHaveErrorInfo();
  const t = e ? {
    type: build_event_js_1.WatchEvent.WATCH_RESULT,
    content: lastWatchRes,
    hotCompile: "hotCompile"
  } : {
    type: build_event_js_1.WatchEvent.WATCH_RESULT,
    content: lastWatchRes
  };
  watchParentPort?.postMessage(t), clearWatchData();
}
function processTsWatchEnd() {
  tsWatchEnd = true, rollupWatchEnd && sendWatchResult();
}
function processRollupWatchEnd(e) {
  rollupWatchEnd = true, !tsWatchEnd && triggerTsWatch || sendWatchResult(e);
}
function updateLastReUsedFiles(e) {
  e.forEach(e => lastReUsedFiles.add("Windows_NT" === os_1.type() ? path_1.posix.join(...e.split(path_1.sep)) : e));
}
function hotReload(e, t, a) {
  e || (isWatchFirst ? (t.forEach(e => a.add(e)), sendHotReloadWatchFileList(t)) : validateWatchScope(t, a));
}
function validateUnsupportedChangedFiles(e, t) {
  for (const a of t) if (e.has(a)) throw arkts_base_log_js_1.ArktsBaseLogger.addLogEvent({
    time: new Date(),
    category: "watch",
    level: "error",
    msg: `${a} has been changed, which is not supported in watch mode. Please rebuild this module manually.`,
    type: constants_js_1.PLUGIN_LOG_TYPE
  }), new Error("Unsupported file changed in watch mode.");
}
function validateWatchScope(e, t) {
  const a = (e = "new files") => {
    arkts_base_log_js_1.ArktsBaseLogger.addLogEvent({
      time: new Date(),
      category: "watch",
      level: "error",
      msg: `Importing ${e} is not supported in hot reload mode. Please rebuild this module manually.`,
      type: constants_js_1.PLUGIN_LOG_TYPE
    });
  };
  if (e.length > t.size) a();else for (const s of e) if (!t.has(s)) return void a(s);
}
function updateTriggerTsWatchWithLastChangedFiles(e) {
  for (const t of e.keys()) if (willTriggerTsWatch(t)) {
    triggerTsWatch = true;
    break;
  }
  triggerTsWatch = undefined === triggerTsWatch || triggerTsWatch, process_1.env.triggerTsWatch = String(triggerTsWatch);
}
function updateCache(e, t, a) {
  watchEnableFileSystemCache = e, watchCache = t, watchCacheDirectory = a;
}
function willTriggerTsWatch(e) {
  return extensionSet.has(path_1.extname(e));
}
function sendHotReloadWatchFileList(e) {
  e && watchParentPort?.postMessage({
    type: build_event_js_1.WatchEvent.WATCH_RESULT,
    content: JSON.stringify({
      isSuccess: !arkts_base_log_js_1.ArktsBaseLogger.checkLoggerHaveErrorInfo(),
      watchFiles: e
    })
  });
}
