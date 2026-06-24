/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/server/daemon-server-lifecycle.js
 */
import * as daemon_const_js_1 from "../const/daemon-const";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as port_util_js_1 from "../util/port-util";
import * as hvigor_daemon_server_js_1 from "./hvigor-daemon-server";
const _log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon");
export class DaemonServerLifecycle {
  async startListen(e) {
    _log.debug(`hvigor daemon: Starting port scan. minPort=${daemon_const_js_1.DAEMON_SERVER_MIN_LISTEN_PORT}, maxPort=${daemon_const_js_1.DAEMON_SERVER_MAX_LISTEN_PORT}`);
    const _ = await (0, port_util_js_1.tryUsePort)(daemon_const_js_1.DAEMON_SERVER_MIN_LISTEN_PORT);
    if (-1 === _) {
      const _ = `hvigor daemon: No available port was found in the range: ${daemon_const_js_1.DAEMON_SERVER_MIN_LISTEN_PORT} - ${daemon_const_js_1.DAEMON_SERVER_MAX_LISTEN_PORT}`;
      _log.debug(_), e.emit(hvigor_daemon_server_js_1.DaemonServerEventName.LISTEN_FAILED, _);
    } else _log.debug(`hvigor daemon: Available port found. port=${_}, hostname=${daemon_const_js_1.DAEMON_SERVER_LOCAL_HOST}`), e.listen(_, daemon_const_js_1.DAEMON_SERVER_LOCAL_HOST);
  }
}
