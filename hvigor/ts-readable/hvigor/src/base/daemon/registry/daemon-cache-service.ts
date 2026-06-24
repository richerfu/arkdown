/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/daemon-cache-service.js
 */
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as daemon_const_js_1 from "../const/daemon-const";
import * as process_info_util_js_1 from "../util/process-info-util";
import * as daemon_info_js_1 from "./daemon-info";
const DEFAULT_LOCK_OPTION = {
  retries: 100,
  update: 1e3
};
export class DaemonCacheService {
  constructor(e, t) {
    this.cachePath = e, fs_extra_1.existsSync(e) || (fs_extra_1.ensureFileSync(e), fs_extra_1.writeJsonSync(e, {})), this._log = t;
  }
  set(e, t) {
    (0, hvigor_common_1.lockSync)(this.cachePath, DEFAULT_LOCK_OPTION);
    const o = this.getAll();
    o[e] = {
      ...t,
      lastUsedTime: Date.now()
    }, fs_extra_1.writeJsonSync(this.cachePath, o), (0, hvigor_common_1.unlockSync)(this.cachePath);
  }
  close() {}
  get(e) {
    return this.getAll()[e] || {};
  }
  remove(e) {
    (0, hvigor_common_1.lockSync)(this.cachePath, DEFAULT_LOCK_OPTION);
    const t = this.getAll();
    delete t[e], fs_extra_1.writeJsonSync(this.cachePath, t), (0, hvigor_common_1.unlockSync)(this.cachePath);
  }
  size() {
    return Object.entries(fs_extra_1.readJsonSync(this.cachePath)).length;
  }
  initialize() {}
  getAll() {
    try {
      const e = fs_extra_1.readJsonSync(this.cachePath);
      return Object.entries(e).length > daemon_const_js_1.DAEMON_INFO_IN_REGISTRY_MAX_LIMIT && (this._log.debug("Daemon info over limit. Clean daemon info registry."), Object.entries(e).forEach(([t, o]) => {
        o.state !== daemon_info_js_1.DaemonState.BROKEN && o.state !== daemon_info_js_1.DaemonState.STOPPED || this.removeUselessInfoInDaemonRegistry(o, e, t);
      })), e;
    } catch (e) {
      return this._log.debug(e), this._log.debug("The registry information is abnormal, the daemon information will be cleared."), fs_extra_1.removeSync(this.cachePath), fs_extra_1.writeJsonSync(this.cachePath, {}), {};
    }
  }
  removeUselessInfoInDaemonRegistry(e, t, o) {
    const s = e.pid;
    (0, process_info_util_js_1.isHvigorDaemonProcess)(s) ? process.kill(s, "SIGKILL") : this._log.debug(`Client check process is not a hvigor daemon or not exist. daemonPid=${s}`), this._log.debug(`Remove daemon info. The information in the registry has exceeded the limit. daemonPid=${s}`), delete t[o];
  }
}
