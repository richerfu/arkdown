/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/server/daemon-server-listener-registry.js
 */
import process_1 from "process";
import * as exit_js_1 from "../../boot/hooks/exit";
import * as build_event_js_1 from "../../common/daemon-protocol/build-event";
import * as common_enum_js_1 from "../../common/daemon-protocol/common-enum";
import * as message_js_1 from "../../common/daemon-protocol/message";
import * as daemon_const_js_1 from "../const/daemon-const";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as daemon_process_lifecycle_js_1 from "../process/daemon-process-lifecycle";
import * as daemon_info_js_1 from "../registry/daemon-info";
import * as daemon_info_service_js_1 from "../registry/daemon-info-service";
import * as daemon_server_registry_js_1 from "../registry/daemon-server-registry";
import * as session_manager_js_1 from "../session/session-manager";
import * as daemon_server_socket_listener_registry_js_1 from "../socket/daemon-server-socket-listener-registry";
import * as print_util_js_1 from "../util/print-util";
const _log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon");
export class DaemonServerListenerRegistry {
  registryAllListener(e) {
    this.onListening(e), this.onConnection(e), this.onError(e), this.onCloseConnection(e), this.onListenFailed(e), this.onCloseServer(e);
  }
  onConnection(e) {
    e.onConnection(async s => {
      session_manager_js_1.defaultSessionManager.initSession(s);
      const o = session_manager_js_1.defaultSessionManager.getLength();
      if (_log.debug(`hvigor daemon: Socket is connected. socketId=${s.id}, connectionCount=${o}, maxLimit=${daemon_const_js_1.DAEMON_SOCKETS_MAX_LIMIT}`), o > daemon_const_js_1.DAEMON_SOCKETS_MAX_LIMIT) {
        const e = "hvigor daemon：The number of hvigor daemon connections has reached the upper limit, this socket will be disconnect. Please wait for one of the connections to be disconnected and then try again.";
        _log.debug(`hvigor daemon: Connection limit exceeded. socketId=${s.id}, currentCount=${o}, limit=${daemon_const_js_1.DAEMON_SOCKETS_MAX_LIMIT}`), s.emit(build_event_js_1.BuildEvent.BUILD_STATUS, (0, message_js_1.createBuildStatus)(common_enum_js_1.BuildStatus.REJECT, -1, e)), s.disconnect(true);
      }
      new daemon_server_socket_listener_registry_js_1.DaemonServerSocketListenerRegistry(s).registryBaseListener(e), _log.debug(`hvigor daemon: daemonState=${(0, daemon_process_lifecycle_js_1.getCurrentProcessState)()}\n      daemonInfo=${JSON.stringify(daemon_server_registry_js_1.defaultDaemonServerRegistry.getInfos())}`), (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() !== daemon_info_js_1.DaemonState.IDLE && (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() !== daemon_info_js_1.DaemonState.CANCELED || (_log.debug(`hvigor daemon: Process state changed from ${(0, daemon_process_lifecycle_js_1.getCurrentProcessState)()} to HALF_BUSY`), (0, daemon_process_lifecycle_js_1.setCurrentProcessState)(daemon_info_js_1.DaemonState.HALF_BUSY));
    });
  }
  onCloseConnection(e) {
    e.onCloseConnection(async e => {
      var s;
      e.id === (null === (s = session_manager_js_1.defaultSessionManager.getActiveSocket()) || undefined === s ? undefined : s.id) && session_manager_js_1.defaultSessionManager.resetActiveSocket(), session_manager_js_1.defaultSessionManager.endSession(e);
      const o = session_manager_js_1.defaultSessionManager.getSockets().length;
      _log.debug(`hvigor daemon: Socket will be closed. socketId=${e.id}, connectionCount=${o}`), 0 === o && (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() === daemon_info_js_1.DaemonState.HALF_BUSY && (0, daemon_process_lifecycle_js_1.setCurrentProcessState)(daemon_info_js_1.DaemonState.IDLE);
    });
  }
  onCloseServer(e) {
    e.onCloseServer(e => {
      _log.debug(`hvigor daemon: connectionCount=${session_manager_js_1.defaultSessionManager.getLength()}`), e && (_log.error(`hvigor daemon: ${e}`), e.stack && _log.error(e.stack)), [...session_manager_js_1.defaultSessionManager.getSockets()].forEach(s => {
        s.emit(build_event_js_1.BuildEvent.OUTPUT, e.stack), s.emit(common_enum_js_1.BuildStatus.CLOSE, (0, message_js_1.createBuildStatus)(common_enum_js_1.BuildStatus.CLOSE, 0));
      });
    });
  }
  onError(e) {
    e.onError(e => {
      _log.error(`Server error: ${e.message}`), (0, exit_js_1.exit)(-1);
    });
  }
  onListening(e) {
    e.onListening(() => {
      const s = e.getHttpServer().address();
      _log.debug(`hvigor daemon: listening connection. address=${s.address}, port=${s.port}.`);
      const o = daemon_info_service_js_1.defaultDaemonInfoService.createDefaultDaemonInfo(s, e.getSessionId());
      daemon_server_registry_js_1.defaultDaemonServerRegistry.setInfo(o.keyId, o), undefined !== process_1.send && process_1.send(JSON.stringify(o)), (0, print_util_js_1.overridePrintWrite)();
    });
  }
  onListenFailed(e) {
    e.onListenFailed(e => {
      undefined !== process_1.send && process_1.send(`${daemon_const_js_1.DAEMON_LISTEN_FAILED_REASON_PREFIX}${e}`);
    });
  }
}
