/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/cluster/master-process-boot.js
 */
import cluster_1 from "cluster";
import * as log4js_1 from "log4js";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as daemon_process_lifecycle_js_1 from "../process/daemon-process-lifecycle";
import * as daemon_server_lifecycle_js_1 from "../server/daemon-server-lifecycle";
import * as daemon_server_listener_registry_js_1 from "../server/daemon-server-listener-registry";
import * as hvigor_daemon_server_js_1 from "../server/hvigor-daemon-server";
import * as session_id_js_1 from "../session/session-id";
import * as cacl_exec_argv_util_js_1 from "../../../common/util/cacl-exec-argv-util";
import process_1 from "process";
const _log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon");
export function masterProcessBoot() {
  var e, r;
  if (cluster_1.isPrimary) {
    const o = null !== (e = process_1.env._logLevel) && undefined !== e ? e : log4js_1.levels.INFO;
    (0, hvigor_daemon_log_js_1.configureDaemon)(), _log.debug("hvigor daemon: Daemon logger configured successfully");
    const s = log4js_1.levels.getLevel(o);
    undefined !== s && (0, hvigor_log_js_1.evaluateLogLevel)(s, ["daemon"]), _log.debug(`hvigor daemon: Set daemon process log level, logLevel=${o}`), process_1.env._sessionId || _log.errorMessageExit("hvigor daemon: Session id cannot be empty. Daemon process will exit."), _log.debug(`hvigor daemon: Decrypting session id, session lengthId=${null === (r = process_1.env._sessionId) || undefined === r ? undefined : r.length}`);
    const i = session_id_js_1.SessionIdHelper.decryptPwdByDefaultMaterial(process_1.env._sessionId, _log);
    _log.debug(`hvigor daemon: Session id decrypted successfully, sessionId prefix=${i.substring(0, 8)}...`), _log.debug("hvigor daemon: Initializing cluster"), initCluster(_log);
    const t = hvigor_daemon_server_js_1.HvigorDaemonServer.getInstance(i);
    _log.debug(`hvigor daemon: HvigorDaemonServer instance created, sessionId prefix=${i.substring(0, 8)}...`), (0, daemon_process_lifecycle_js_1.registryProcessListener)(t), _log.debug("hvigor daemon: Process listener registered");
    const n = new daemon_server_lifecycle_js_1.DaemonServerLifecycle();
    new daemon_server_listener_registry_js_1.DaemonServerListenerRegistry().registryAllListener(t), _log.debug("hvigor daemon: Server listener registered"), _log.debug("hvigor daemon: Starting server listen"), n.startListen(t).then(() => {
      _log.debug("hvigor daemon: Server started listening, setting up timers"), (0, daemon_process_lifecycle_js_1.setProcessTimeOutTimer)(t), (0, daemon_process_lifecycle_js_1.setProcessRemovedFromRegistryTimer)(t);
    });
  }
}
function newWorkerProcess(e = false) {
  var r;
  process_1.execArgv = (0, cacl_exec_argv_util_js_1.calcChildExecArgv)(e), _log.debug(`hvigor daemon: Creating new worker process. trigByConfigChange=${e}, execArgv=${JSON.stringify(process_1.execArgv)}`), cluster_1.setupPrimary({
    execArgv: process_1.execArgv
  });
  const o = cluster_1.fork();
  _log.debug(`hvigor daemon: Worker process forked. workerId=${null == o ? undefined : o.id}, pid=${null === (r = o.process) || undefined === r ? undefined : r.pid}`), o.process.stdout.pipe(process_1.stdout), o.process.stderr.pipe(process_1.stderr);
}
function trigByConfigChange(e) {
  return e.exitedAfterDisconnect;
}
function initCluster(e) {
  cluster_1.on("setup", r => {
    e.debug("hvigor daemon: worker process fork option:"), e.debug(r);
  }), cluster_1.on("exit", (r, o, s) => {
    e.debug(`hvigor daemon: worker ${r.process.pid} died (${s || o}).`), e.debug(`hvigor daemon: worker ${r.process.pid} exited with code ${o} and signal ${s}`), r.exitedAfterDisconnect && e.debug(`hvigor daemon: worker ${r.process.pid} requested restart proactively.`), cluster_1.workers && 0 !== Object.keys(cluster_1.workers).length || (e.debug("hvigor daemon: no worker alive. Start a new worker process."), newWorkerProcess(trigByConfigChange(r)));
  }), cluster_1.on("online", r => {
    e.debug(`hvigor daemon: worker ${r.process.pid} with worker id ${r.id} is online.`), r.on("disconnect", () => {
      e.debug(`hvigor daemon: worker ${r.process.pid} with worker id ${r.id} is disconnected.`);
    }), r.on("exit", o => {
      e.debug(`hvigor daemon: worker ${r.process.pid} with worker id ${r.id} exit with code ${o}.`);
    }), r.on("error", o => {
      e.debug(`hvigor daemon: worker ${r.process.pid} with worker id ${r.id} has an error: ${o}`), o.stack && e.debug(o.stack);
    });
  }), cluster_1.workers && 0 !== Object.keys(cluster_1.workers).length || (cluster_1.setupPrimary({
    windowsHide: true,
    silent: true
  }), newWorkerProcess());
}
