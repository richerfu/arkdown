/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/util/port-util.js
 */
import net_1 from "net";
import * as daemon_const_js_1 from "../const/daemon-const";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as daemon_server_registry_js_1 from "../registry/daemon-server-registry";
const log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon");
function isPortInUsed(e) {
  return new Promise(r => {
    const o = net_1.createServer().listen(e, daemon_const_js_1.DAEMON_SERVER_LOCAL_HOST);
    o.on("listening", function () {
      o.close(), r(e);
    }), o.on("error", function (e) {
      r(e);
    });
  });
}
export async function tryUsePort(e) {
  const r = daemon_server_registry_js_1.defaultDaemonServerRegistry.getAllPortInUsed();
  for (; r.includes(e);) if (log.debug(`${e} is occupied by hvigor daemon.`), ++e > daemon_const_js_1.DAEMON_SERVER_MAX_LISTEN_PORT) return -1;
  return (await isPortInUsed(e)) instanceof Error ? (log.debug(`${e} is occupied by other process.`), ++e > daemon_const_js_1.DAEMON_SERVER_MAX_LISTEN_PORT ? -1 : tryUsePort(e)) : e;
}
