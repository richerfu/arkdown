/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/connection/default-daemon-connector.js
 */
import process_1 from "process";
import * as socket_io_client_1 from "socket.io-client";
import * as daemon_const_js_1 from "../const/daemon-const";
import * as hvigor_daemon_client_log_js_1 from "../log/hvigor-daemon-client-log";
import * as daemon_process_factory_js_1 from "../process/daemon-process-factory";
import * as daemon_client_registry_js_1 from "../registry/daemon-client-registry";
import * as daemon_info_js_1 from "../registry/daemon-info";
import * as session_id_js_1 from "../session/session-id";
const log = hvigor_daemon_client_log_js_1.HvigorDaemonClientLogger.getLogger("daemon-client"),
  SocketEvent = {
    EVENT_CONNECT: "connect",
    EVENT_DISCONNECT: "disconnect",
    EVENT_CONNECT_ERROR: "connect_error"
  };
export class DefaultDaemonConnector {
  async getDaemonConnection() {
    undefined !== daemon_process_factory_js_1.defaultDaemonServerFactory.createDaemonPromise && (await daemon_process_factory_js_1.defaultDaemonServerFactory.createDaemonPromise);
    const e = await daemon_process_factory_js_1.defaultDaemonServerFactory.getProjectCompatibleIdleDaemon();
    if (undefined === e) return void log.debug("hvigor client: No idle daemon available, will use no-daemon mode");
    log.debug(`hvigor client: Found idle daemon. pid=${null == e ? undefined : e.pid}, port=${null == e ? undefined : e.port}, state=${null == e ? undefined : e.state}`);
    try {
      process_1.kill(e.pid, 0), log.debug(`hvigor client: Daemon process ${null == e ? undefined : e.pid} is alive`);
    } catch (o) {
      log.debug(`hvigor client: Registry info is incorrect: Cannot find a valid process by process id ${null == e ? undefined : e.pid}.`);
      const n = "process is broken by user or operating system.";
      return daemon_client_registry_js_1.defaultDaemonClientRegistry.updateDaemonProcessState(e, daemon_info_js_1.DaemonState.BROKEN, n), this.getDaemonConnection();
    }
    if (e.state === daemon_info_js_1.DaemonState.BUSY || e.state === daemon_info_js_1.DaemonState.STOP_REQ) {
      const e = `hvigor client: Current daemon process is busy, cannot accept new build request. You can run hvigorw --status-daemon to check daemon status, or rerun build with --no-daemon param. Delete ${daemon_const_js_1.DEFAULT_DAEMON_REGISTRY_PATH} is also a solution, but this might stop current build action.`;
      log.error(e), log.clientErrorLink(), process_1.exit(-1);
    }
    process_1.exitCode = -1;
    const o = {
      transports: ["websocket"]
    };
    if (e.sessionId) {
      const n = session_id_js_1.SessionIdHelper.decryptPwdByDefaultMaterial(e.sessionId, log);
      o.path = `/${n}`;
    }
    log.debug(`hvigor client: connect to hvigor daemon, daemonInfo=${JSON.stringify(e)}`);
    const n = (0, socket_io_client_1.io)(`ws://${daemon_const_js_1.DAEMON_SERVER_LOCAL_HOST}:${e.port}`, o);
    return n.on(SocketEvent.EVENT_CONNECT, this.onConnect.bind(this)), n.on(SocketEvent.EVENT_DISCONNECT, this.onClose.bind(this, e)), n.on(SocketEvent.EVENT_CONNECT_ERROR, this.onConnectError.bind(this, e)), n;
  }
  onConnect(...e) {
    log.debug("hvigor client: Client connected.");
  }
  onClose(e, o) {
    switch (log.debug("hvigor client: Connection between client and daemon is disconnected:", o), o) {
      case "ping timeout":
      case "transport error":
      case "transport close":
        try {
          process_1.kill(e.pid, 0);
        } catch (o) {
          log.error("hvigor client: This is an error caused by hvigor daemon. The daemon process was stopped."), log.daemonErrorLink(e.pid);
          break;
        }
        log.error(`hvigor client: This is an error caused by hvigor daemon. The connection between hvigor client and build daemon is disconnected. Reason: ${o}`), log.daemonErrorLink(e.pid);
    }
    process_1.exit();
  }
  onConnectError(e, o) {
    log.error("hvigor client: Connection between client and daemon is disconnected with a connect error:", o), log.error("hvigor client: This is an error caused by hvigor daemon."), log.daemonErrorLink(e.pid), process_1.exit();
  }
}
