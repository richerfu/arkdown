/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/default-daemon-registry.js
 */
import * as daemon_const_js_1 from "../const/daemon-const";
import * as daemon_cache_service_js_1 from "./daemon-cache-service";
import * as daemon_info_js_1 from "./daemon-info";
export class DefaultDaemonRegistry {
  constructor(e) {
    this._log = e, this.daemonCacheService = new daemon_cache_service_js_1.DaemonCacheService(daemon_const_js_1.DEFAULT_DAEMON_REGISTRY_PATH, e);
  }
  getIdles() {
    return this.getInfos().filter(e => e.state === daemon_info_js_1.DaemonState.IDLE);
  }
  getAlive() {
    return this.getInfos().filter(e => e.state !== daemon_info_js_1.DaemonState.STOPPED && e.state !== daemon_info_js_1.DaemonState.BROKEN && e.state !== daemon_info_js_1.DaemonState.BUSY);
  }
  getInfo(e) {
    return {
      ...this.daemonCacheService.get(e),
      keyId: e
    };
  }
  getInfos() {
    return Object.entries(this.daemonCacheService.getAll()).map(([e, t]) => ({
      ...t,
      keyId: e
    }));
  }
  getDaemonInfoByPid(e) {
    return this.getInfos().filter(t => t.pid === e).pop();
  }
  setInfo(e, t) {
    this.daemonCacheService.set(e, t);
  }
}
