/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/process/daemon-process-factory.js
 */
import * as child_process_1 from "child_process";
import * as crypto_1 from "crypto";
import path_1 from "path";
import process_1 from "process";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as util_js_1 from "../../../cli/wrapper/util";
import * as const_js_1 from "../../../common/const/const";
import * as cacl_exec_argv_util_js_1 from "../../../common/util/cacl-exec-argv-util";
import * as path_util_js_1 from "../../../common/util/path-util";
import pretty_hrtime_js_1 from "../../../common/util/pretty-hrtime";
import * as hvigor_config_reader_js_1 from "../../util/hvigor-config-reader";
import * as daemon_const_js_1 from "../const/daemon-const";
import * as hvigor_daemon_client_log_js_1 from "../log/hvigor-daemon-client-log";
import * as daemon_client_registry_js_1 from "../registry/daemon-client-registry";
import * as daemon_info_js_1 from "../registry/daemon-info";
import * as session_id_js_1 from "../session/session-id";
import * as process_info_util_js_1 from "../util/process-info-util";
const log = hvigor_daemon_client_log_js_1.HvigorDaemonClientLogger.getLogger("daemon-client");
export class DaemonProcessFactory {
  constructor() {
    this.createDaemonPromise = undefined, this.childExecArgv = (0, cacl_exec_argv_util_js_1.calcChildExecArgv)();
  }
  async getProjectCompatibleIdleDaemon(e = true) {
    const o = daemon_client_registry_js_1.defaultDaemonClientRegistry.findProjectDaemon();
    if (undefined === o) {
      if (log.debug(`hvigor client: Cannot find a valid daemon for project, projectPath=${process_1.cwd()}`), daemon_client_registry_js_1.defaultDaemonClientRegistry.isAvailableDaemonOverLimit()) {
        const o = this.cleanupBrokenBusyDaemons();
        if (log.debug(`hvigor client: Cleaned up ${o} stopped busy daemon(s).`), !daemon_client_registry_js_1.defaultDaemonClientRegistry.isAvailableDaemonOverLimit()) return this.createNewIdleDaemon(e);
        const t = daemon_client_registry_js_1.defaultDaemonClientRegistry.findIdleDaemonWithLRU();
        if (undefined === t) return void log.debug("hvigor client: Cannot find another idle daemon. This build will be executed with no-daemon mode.");
        {
          const e = t.pid;
          return log.debug(`hvigor client: Idle daemon (pid=${e}) will be stopped, and a new daemon will be created.`), this.killOldProcessAndCreateNew(t, "daemon process exceeds the upper limit.");
        }
      }
      return this.createNewIdleDaemon(e);
    }
    const t = function (e) {
        return e.hvigorVersion !== const_js_1.CUR_HVIGOR_VERSION;
      }(o),
      r = function (e, o) {
        log.debug(`check node env with process execArgv, execArgv=${o}`);
        const t = require.resolve(const_js_1.HVIGOR_ENGINE_PACKAGE_NAME);
        return e.nodeVersion !== process_1.version || e.nodeParams !== (0, hvigor_common_1.hash)(`${t}${(0, util_js_1.getCurrentProjectCacheDirHash)()}${o.toString()}`);
      }(o, this.childExecArgv),
      i = function (e) {
        return path_util_js_1.PathUtil.getHvigorCacheDir() !== e.cachePath;
      }(o);
    return t || r || i ? (log.info(`hvigor client: The argument passed to Node.js or hvigor version have changed and a new daemon will be created to replace the existing daemon. Reason:{isHvigorVersionChanged: ${t}, isNodeEnvChanged: ${r}, isCachePathChanged: ${i}}`), this.killOldProcessAndCreateNew(o, "stopped by hvigor, runtime has changed.")) : o;
  }
  async killOldProcessAndCreateNew(e, o) {
    return daemon_client_registry_js_1.defaultDaemonClientRegistry.updateDaemonProcessState(e, daemon_info_js_1.DaemonState.STOPPED, o), this.createNewIdleDaemon();
  }
  cleanupBrokenBusyDaemons() {
    const e = this.getBusyDaemons();
    let o = 0;
    for (const t of e) (0, process_info_util_js_1.isDaemonProcessAlive)(t) || (log.debug(`hvigor client: Busy daemon (pid=${t.pid}) is not alive, updating state to BROKEN.`), daemon_client_registry_js_1.defaultDaemonClientRegistry.updateDaemonProcessState(t, daemon_info_js_1.DaemonState.BROKEN, "daemon process is dead but state was busy"), o++);
    return o;
  }
  getBusyDaemons() {
    return daemon_client_registry_js_1.defaultDaemonClientRegistry.getInfos().filter(e => e.state === daemon_info_js_1.DaemonState.BUSY);
  }
  async createNewIdleDaemon(e = true) {
    log.info("hvigor client: Starting hvigor daemon.");
    const o = process_1.hrtime();
    let t;
    const r = (0, crypto_1.randomBytes)(32).toString("hex"),
      i = session_id_js_1.SessionIdHelper.encryptPwdByDefaultMaterial(r, log),
      n = hvigor_config_reader_js_1.HvigorConfigReader.getNodeParamFromProcessArgv(),
      s = (0, child_process_1.fork)(path_1.resolve(__dirname, "daemon-process-boot-script.js"), [log.getLevel().toString(), i, ...n], {
        stdio: ["pipe", "pipe", "pipe", "ipc"],
        detached: true,
        env: {
          ...process_1.env,
          _logLevel: log.getLevel().toString(),
          _sessionId: i
        },
        execArgv: this.childExecArgv
      });
    let a;
    return this.createDaemonPromise = new Promise(e => {
      s.once("message", r => {
        r.startsWith(daemon_const_js_1.DAEMON_LISTEN_FAILED_REASON_PREFIX) ? (log.warn(`hvigor client: daemon failed to listen on the port. Build will be started in no-daemon mode. Reason: ${r}`), t = undefined) : (t = JSON.parse(r), log.info(`Hvigor Daemon started in ${(0, pretty_hrtime_js_1)(process_1.hrtime(o))}`), log.debug(`A new daemon has been created successfully and is listening for requests, daemonInfo=${r}`)), e();
      }), a = this.getWaitConnectTimeout(s, e);
    }), e && (await this.createDaemonPromise), clearTimeout(a), s.unref(), t;
  }
  getWaitConnectTimeout(e, o) {
    return setTimeout(() => {
      e.off("message", () => {
        log.warn("hvigor client: Failed to start hvigor daemon. The build will be started in no-daemon mode."), e.pid ? (log.warn("hvigor client: This should be an error while initializing hvigor daemon."), log.daemonWarnLink(e.pid)) : log.warn("hvigor client: This should be an error that hvigor daemon didn't start.");
      }), o();
    }, 1e4);
  }
  createDaemonFork() {
    this.getProjectCompatibleIdleDaemon(false);
  }
}
export const defaultDaemonServerFactory = new DaemonProcessFactory();
