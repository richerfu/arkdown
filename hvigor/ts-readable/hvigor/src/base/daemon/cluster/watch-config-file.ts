/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/cluster/watch-config-file.js
 */
import cluster_1 from "cluster";
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as chokidar_1 from "chokidar";
import * as const_js_1 from "../../../common/const/const";
import * as path_const_js_1 from "../../../common/const/path-const";
import * as event_id_options_js_1 from "../../internal/lifecycle/event/event-id-options";
import * as hvigor_process_js_1 from "../../internal/lifecycle/hvigor-process";
import * as hvigor_config_reader_js_1 from "../../util/hvigor-config-reader";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
const _log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon");
let watcher,
  isRunning = false;
const hvigorFiles = [];
let hvigorConfigWatcher;
function disconnectHvigorConfig() {
  var e, o;
  const r = process.execArgv,
    i = r.findIndex(e => e.startsWith("--max-old-space-size=")),
    n = (-1 !== i ? Number(r[i].split("=")[1]) : hvigor_config_reader_js_1.defaultOptions.maxOldSpaceSize) !== hvigor_config_reader_js_1.HvigorConfigReader.getMaxOldSpaceSize(true),
    s = r.findIndex(e => e.startsWith("--max-semi-space-size=")),
    t = (-1 !== s ? Number(r[s].split("=")[1]) : hvigor_config_reader_js_1.defaultOptions.maxSemiSpaceSize) !== hvigor_config_reader_js_1.HvigorConfigReader.getMaxSemiSpaceSize(true),
    _ = r.findIndex(e => e.startsWith("--expose-gc")),
    c = -1 !== _ ? null === (o = null === (e = hvigor_config_reader_js_1.HvigorConfigReader.getHvigorConfig()) || undefined === e ? undefined : e.nodeOptions) || undefined === o ? undefined : o.exposeGC : hvigor_config_reader_js_1.defaultOptions.exposeGC,
    d = -1 === _ && false !== c || -1 !== _ && false === c,
    a = process.execArgv.some(e => e === const_js_1.ENABLE_SOURCE_MAPS),
    g = hvigor_config_reader_js_1.HvigorConfigReader.getStacktrace(true);
  (n || t || d || a !== g) && (_log.debug(`Worker ${cluster_1.worker.id} detected changes in 'hvigor-config.json5' nodeOptions.maxOldSpaceSize or maxSemiSpaceSize\n        or exposeGC or debugging.stacktrace. Path ${path_1} changed. Will exit and start another worker process.`), cluster_1.worker.isConnected() && (_log.debug("worker isConnected start disconnect"), cluster_1.worker.disconnect(), cluster_1.worker.kill()));
}
export function addConfigDeps() {
  const e = getModuleDependencies(hvigorFiles).filter(e => {
    const o = path_1.relative(path_const_js_1.HVIGOR_USER_HOME, e);
    return !(o && !o.startsWith("..") && path_1.isAbsolute(o) && !e.includes("hvigorfile.ts") && !e.includes("hvigorconfig.ts"));
  });
  cluster_1.isWorker && (_log.debug("watch files:", e), watcher.add(e));
}
export function addModuleDependency(e) {
  hvigorFiles.includes(e) || hvigorFiles.push(e);
}
export function setRunning(e) {
  isRunning = e;
}
function getModuleDependencies(e) {
  const o = e.map(e => require.resolve(e));
  return e.forEach(e => {
    const r = require.resolve(e),
      i = require.cache[r];
    undefined !== i && traverseDependencies(i, o);
  }), o;
}
function traverseDependencies(e, o) {
  e.children.forEach(e => {
    o.includes(e.filename) || (o.push(e.filename), traverseDependencies(e, o));
  });
}
cluster_1.isWorker && (watcher = (0, chokidar_1.watch)([], {
  usePolling: (0, hvigor_common_1.isWindows)(),
  interval: 1e3
}), watcher.on("change", e => {
  const o = () => {
    _log.debug(`worker ${cluster_1.worker.id} detected hvigorfile's dependency changed. Path ${e} changed. Will exit and start another worker process.`), cluster_1.worker.isConnected() && (_log.debug("worker isConnected start disconnect"), cluster_1.worker.disconnect(), cluster_1.worker.kill());
  };
  isRunning ? (hvigor_process_js_1.hvigorProcess.once(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FINISHED, o), hvigor_process_js_1.hvigorProcess.once(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED, o)) : o();
}), hvigorConfigWatcher = (0, chokidar_1.watch)([path_1.resolve(path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME)], {
  usePolling: (0, hvigor_common_1.isWindows)(),
  interval: 1e3
}), hvigorConfigWatcher.on("change", e => {
  isRunning ? (hvigor_process_js_1.hvigorProcess.once(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FINISHED, disconnectHvigorConfig), hvigor_process_js_1.hvigorProcess.once(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED, disconnectHvigorConfig)) : disconnectHvigorConfig();
}));
