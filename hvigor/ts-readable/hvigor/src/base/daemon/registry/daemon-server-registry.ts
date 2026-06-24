/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/daemon-server-registry.js
 */
import process_1 from "process";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as daemon_info_js_1 from "./daemon-info";
import * as default_daemon_registry_js_1 from "./default-daemon-registry";
const _log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon");
class DaemonServerRegistry extends default_daemon_registry_js_1.DefaultDaemonRegistry {
  constructor() {
    super(_log);
  }
  updateDaemonProcessState(e, o) {
    const t = this.getDaemonInfoByPid(process_1.pid);
    if (!t) return void _log.debug("hvigor daemon: Cannot transform process state. Unable to find daemon info from registry.");
    const r = {
      ...t,
      state: e,
      info: o
    };
    _log.debug(`hvigor daemon: Update daemon info in registry. daemonInfo=${JSON.stringify(r)}`), this.setInfo(t.keyId, r);
  }
  getAllPortInUsed() {
    return this.getInfos().filter(e => e.state !== daemon_info_js_1.DaemonState.BROKEN && e.state !== daemon_info_js_1.DaemonState.STOPPED).map(e => e.port);
  }
}
export const defaultDaemonServerRegistry = new DaemonServerRegistry();
