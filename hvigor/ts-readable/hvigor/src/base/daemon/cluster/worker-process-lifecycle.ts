/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/cluster/worker-process-lifecycle.js
 */
import cluster_1 from "cluster";
import process_1 from "process";
import * as log4js_1 from "log4js";
import * as index_js_1 from "../../boot/index";
import * as build_event_js_1 from "../../common/daemon-protocol/build-event";
import * as global_core_parameters_js_1 from "../../internal/data/global-core-parameters";
import * as global_data_js_1 from "../../internal/data/global-data";
import * as watch_worker_js_1 from "../../internal/pool/worker-pool/watch-worker";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as watch_config_file_js_1 from "./watch-config-file";
import * as worker_process_event_id_js_1 from "./worker-process-event-id";
import * as worker_pool_factory_js_1 from "../../internal/pool/worker-pool/worker-pool-factory";
function addExitHandler(e) {
  let r = false;
  const o = (o, t) => {
    var s, i;
    if (r) return;
    r = true;
    let n = false;
    t && (e.debug(`hvigor worker: Exit with error: ${t}`), t.stack && e.debug(`hvigor worker: Stack: ${t.stack}`), process_1.send({
      type: worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FAILED,
      content: null !== (i = null !== (s = t.stack) && undefined !== s ? s : t.message) && undefined !== i ? i : t
    })), setTimeout(() => {
      e.debug("hvigor worker: Set process exit timeout in 1000 ms."), e.debug(`hvigor worker: worker process exit. exitCode=${o}`), n || (n = true, setImmediate(() => process_1.exit(o)));
    }, 1e3);
  };
  process_1.on("exit", r => {
    e.debug(`hvigor worker: worker process exit (${r}).`), o(r);
  }), process_1.on("SIGHUP", () => o(129)), process_1.on("SIGINT", () => o(130)), process_1.on("SIGTERM", () => o(143)), process_1.on("SIGBREAK", () => o(149)), process_1.on("uncaughtException", e => o(1, e)), process_1.on("unhandledRejection", e => o(1, e));
}
function initWorkerLogger() {
  var e;
  const r = null !== (e = process_1.env._logLevel) && undefined !== e ? e : log4js_1.levels.INFO;
  (0, hvigor_daemon_log_js_1.configureDaemon)();
  const o = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon"),
    t = log4js_1.levels.getLevel(r);
  return (0, hvigor_log_js_1.evaluateLogLevel)(t, ["daemon"]), o;
}
function onOnline(e, r) {
  e.on("online", () => {
    r.debug(`hvigor worker: worker ${process_1.pid} with worker id ${e.id} is online.`);
  });
}
function onDisconnect(e, r) {
  e.on("disconnect", () => {
    r.debug(`hvigor worker: worker ${process_1.pid} with worker id ${e.id} is disconnected.`);
  });
}
function onExit(e, r) {
  e.on("exit", (o, t) => {
    r.debug(`hvigor worker: worker ${process_1.pid} with worker id ${e.id} exit. (${o || t})`);
  });
}
function onError(e, r) {
  process_1.on("error", o => {
    r.error(`hvigor worker: worker ${process_1.pid} with worker id ${e.id} has an error. Error: ${o}`), o.stack && r.error(o.stack);
  });
}
function onBuild(e, r) {
  e.on("message", async e => {
    var o, t, s, i, n;
    if (e.type !== build_event_js_1.BuildEvent.COMMON_BUILD) return;
    (0, watch_config_file_js_1.setRunning)(true);
    const l = e.content;
    r.debug(`hvigor worker: Received build request. task=${(null === (o = null == l ? undefined : l._) || undefined === o ? undefined : o[0]) || "default"}, hotCompile=${null == l ? undefined : l.hotCompile}`);
    try {
      if (l.hotCompile && l._.includes("assembleDevHqf")) return r.debug("hvigor worker: Hot compile mode, sending compile message to worker"), void watch_worker_js_1.watchWorker.sendCompileMsgToWorker();
      r.debug("hvigor worker: Refreshing daemon process environment and initializing start data"), (0, global_data_js_1.refreshDaemonProcessEnv)(l), (0, global_data_js_1.initStartData)(l);
    } catch (e) {
      return r.error(e), void process_1.send({
        type: worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FAILED,
        content: null !== (s = null !== (t = e.stack) && undefined !== t ? t : e.message) && undefined !== s ? s : e
      });
    }
    (0, hvigor_log_js_1.evaluateLogLevel)(global_core_parameters_js_1.coreParameter.startParams.logLevel, ["daemon"]), worker_pool_factory_js_1.WorkerPoolFactory.getDefaultWorkerPool().updateLogLevel(global_core_parameters_js_1.coreParameter.startParams.logLevel);
    try {
      await (0, index_js_1.boot)(e.content), r.debug("hvigor worker: Build process completed");
    } catch (e) {
      r.error(e), process_1.send({
        type: worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FAILED,
        content: null !== (n = null !== (i = e.stack) && undefined !== i ? i : e.message) && undefined !== n ? n : e
      });
    }
  });
}
function initWorkerProcessListeners(e) {
  const r = cluster_1.worker;
  onOnline(r, e), onDisconnect(r, e), onExit(r, e), onError(r, e), onBuild(r, e);
}
export function registryWorkerProcessListener() {
  if (cluster_1.isWorker) {
    const e = initWorkerLogger();
    initWorkerProcessListeners(e), addExitHandler(e);
  }
}
