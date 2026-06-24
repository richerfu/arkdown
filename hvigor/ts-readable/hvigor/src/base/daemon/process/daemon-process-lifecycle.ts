/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/process/daemon-process-lifecycle.js
 */
import * as wdk_1 from "@baize/wdk";
import fs_1 from "fs";
import * as path from "path";
import process_1 from "process";
import * as daemon_const_js_1 from "../const/daemon-const";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as daemon_info_js_1 from "../registry/daemon-info";
import * as daemon_info_service_js_1 from "../registry/daemon-info-service";
import * as daemon_server_registry_js_1 from "../registry/daemon-server-registry";
import * as hvigor_daemon_server_js_1 from "../server/hvigor-daemon-server";
let timeout,
  isExiting = false;
const _log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon");
let currentProcessState = daemon_info_js_1.DaemonState.IDLE;
export function getCurrentProcessState() {
  return currentProcessState;
}
export function setCurrentProcessState(e, t) {
  const r = currentProcessState;
  currentProcessState = e, _log.debug(`hvigor daemon: Process state changed. previousState=${r}, newState=${e}, changeInfo=${t}`), daemon_server_registry_js_1.defaultDaemonServerRegistry.updateDaemonProcessState(currentProcessState, t);
}
export function setProcessTimeOutTimer(e) {
  timeout = setInterval(() => {
    getCurrentProcessState() === daemon_info_js_1.DaemonState.IDLE ? (_log.debug("The process will exit automatically because it has been idle for more than 3 hours."), setCurrentProcessState(daemon_info_js_1.DaemonState.STOPPED, "process idle timeout"), handleExit(-1, e)) : _log.debug(`The process has not been used for more than 3 hours,' +\n        ' but will not exit because the current status is ${getCurrentProcessState()}`);
  }, daemon_const_js_1.DAEMON_PROCESS_EXIT_TIME_OUT);
}
export function resetProcessTimeOutTimer(e) {
  undefined !== timeout && clearInterval(timeout), _log.debug("The process timeout timer will be reset."), setProcessTimeOutTimer(e);
}
export function setProcessRemovedFromRegistryTimer(e) {
  setInterval(() => {
    if (!daemon_server_registry_js_1.defaultDaemonServerRegistry.getInfos().some(e => e.pid === process_1.pid)) {
      _log.debug("hvigor daemon: Current process is not found in the registry, the process will automatically exit.");
      const t = "process is broken by user or operating system.",
        r = {
          ...daemon_info_service_js_1.defaultDaemonInfoService.createDefaultDaemonInfo(e.getHttpServer().address(), e.getSessionId()),
          state: daemon_info_js_1.DaemonState.BROKEN,
          info: t
        };
      daemon_server_registry_js_1.defaultDaemonServerRegistry.setInfo(r.keyId, r), handleExit(-1, e);
    } else {
      const t = daemon_server_registry_js_1.defaultDaemonServerRegistry.getDaemonInfoByPid(process_1.pid).state;
      t === daemon_info_js_1.DaemonState.STOPPED || t === daemon_info_js_1.DaemonState.BROKEN ? (_log.debug("hvigor daemon: Current process is stopped or broken. The process will exit."), handleExit(-1, e)) : _log.debug(`hvigor daemon: Check daemon process existed in registry, daemonStatus=${t}.`);
    }
  }, daemon_const_js_1.DAEMON_PROCESS_REMOVED_FROM_REGISTRY_CHECK_TIME);
}
function handleDaemonLog() {
  fs_1.readdirSync(daemon_const_js_1.DAEMON_LOG_PATH).forEach(e => {
    const t = path.resolve(daemon_const_js_1.DAEMON_LOG_PATH, e),
      r = fs_1.statSync(t);
    if (function (e, t) {
      return e.isFile() && daemon_const_js_1.DAEMON_LOG_FILE_NAME_PATTEN.test(t);
    }(r, e) && function (e, t) {
      const r = daemon_server_registry_js_1.defaultDaemonServerRegistry.getDaemonInfoByPid(e),
        o = Date.now(),
        s = t.getTime();
      return undefined === r && o - s > daemon_const_js_1.DAEMON_LOG_FILE_RETENTION_TIME;
    }(function (e) {
      const t = e.indexOf("-"),
        r = e.indexOf(".");
      return r <= t ? -1 : (0, wdk_1.toNumber)(e.substring(t + 1, r));
    }(e), r.mtime)) try {
      fs_1.unlinkSync(t), _log.debug(`Delete log file, logPath=${t}.`);
    } catch (e) {
      _log.debug(`Cannot delete log file. The file may be opened by another program. logPath=${t}, errorMessage=${e.message}`);
    }
  });
}
export function registryProcessListener(e) {
  process_1.on("exit", t => {
    _log.debug("hvigor daemon: Daemon process exit."), getCurrentProcessState() === daemon_info_js_1.DaemonState.STOPPED && getCurrentProcessState() === daemon_info_js_1.DaemonState.BROKEN || setCurrentProcessState(daemon_info_js_1.DaemonState.STOPPED, `stopped by exit event with code: ${t}`), handleExit(t, e);
  });
  const t = (t, r) => {
    setCurrentProcessState(daemon_info_js_1.DaemonState.STOPPED, `stopped by signal: ${t}`), handleExit(t, e, r);
  };
  process_1.on("SIGHUP", () => t(129)), process_1.on("SIGINT", () => t(130)), process_1.on("SIGTERM", () => t(143)), process_1.on("SIGBREAK", () => t(149)), process_1.on("uncaughtException", e => t(1, e)), process_1.on("unhandledRejection", e => t(1, e));
}
function handleExit(e, t, r) {
  if (isExiting) return;
  isExiting = true;
  let o = false;
  r && (_log.error(`hvigor daemon: Exit with error: ${r}`), r instanceof Error && r.stack && _log.error(`hvigor daemon: Stack: ${r.stack}`)), t.emit(hvigor_daemon_server_js_1.DaemonServerEventName.CLOSE_SERVER, r), handleDaemonLog(), setTimeout(() => {
    _log.debug("hvigor daemon: Set process exit timeout in 1000 ms."), _log.debug(`hvigor daemon: Daemon process exit. exitCode=${e}`), o || (o = true, setImmediate(() => process_1.exit(e)));
  }, 1e3);
}
