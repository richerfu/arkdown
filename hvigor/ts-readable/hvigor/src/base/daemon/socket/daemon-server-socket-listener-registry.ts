/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/socket/daemon-server-socket-listener-registry.js
 */
import cluster_1 from "cluster";
import process_1 from "process";
import util_1 from "util";
import * as hvigor_config_loader_js_1 from "../../../common/util/hvigor-config-loader";
import * as build_event_js_1 from "../../common/daemon-protocol/build-event";
import * as common_enum_js_1 from "../../common/daemon-protocol/common-enum";
import * as message_js_1 from "../../common/daemon-protocol/message";
import * as global_core_parameters_js_1 from "../../internal/data/global-core-parameters";
import * as global_data_js_1 from "../../internal/data/global-data";
import * as event_id_options_js_1 from "../../internal/lifecycle/event/event-id-options";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as check_hvigor_config_before_program_js_1 from "../../util/check-hvigor-config-before-program";
import * as time_util_js_1 from "../../util/time-util";
import * as worker_process_event_id_js_1 from "../cluster/worker-process-event-id";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as daemon_process_lifecycle_js_1 from "../process/daemon-process-lifecycle";
import * as daemon_info_js_1 from "../registry/daemon-info";
import * as hvigor_daemon_server_js_1 from "../server/hvigor-daemon-server";
import * as session_manager_js_1 from "../session/session-manager";
export class DaemonServerSocketListenerRegistry {
  constructor(e) {
    this._log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon"), this.socket = e;
  }
  registryBaseListener(e) {
    this.onStartBuild(e), this.onCancel(), this.onStop(), this.onError(), this.onDisconnected(e);
  }
  onStartBuild(e) {
    this.socket.on(build_event_js_1.BuildEvent.COMMON_BUILD, async s => {
      var o, t, i, r, n, _, a, d;
      if (!s) {
        const e = "hvigor daemon: Received client exception type information. Empty build arguments.";
        return this._log.debug(`hvigor daemon: Build rejected - empty arguments. socketId=${null === (o = this.socket) || undefined === o ? undefined : o.id}`), void this.socket.emit(build_event_js_1.BuildEvent.BUILD_STATUS, (0, message_js_1.createBuildStatus)(common_enum_js_1.BuildStatus.REJECT, -1, e));
      }
      const l = process_1.hrtime(),
        c = s;
      this._log.debug(`hvigor daemon: Receive build request from client. socketId=${null === (t = this.socket) || undefined === t ? undefined : t.id}, task=${(null === (i = c._) || undefined === i ? undefined : i[0]) || "default"}`);
      const u = (0, check_hvigor_config_before_program_js_1.checkHvigorConfigBeforeProgram)(c, this._log);
      if (!u.result) {
        this._log.debug(`hvigor daemon: Build rejected - hvigor config validation failed. socketId=${null === (r = this.socket) || undefined === r ? undefined : r.id}`);
        const e = global_core_parameters_js_1.coreParameter.startParams.printStackTrace && u.stack ? `${u.message}\n\n${u.stack}` : u.message;
        this.socket.emit(build_event_js_1.BuildEvent.BUILD_STATUS, (0, message_js_1.createBuildStatus)(common_enum_js_1.BuildStatus.REJECT, -1, e));
      }
      try {
        (0, global_data_js_1.initStartData)(c);
      } catch (e) {
        return this._log.error(e), void this.socket.emit(build_event_js_1.BuildEvent.BUILD_STATUS, (0, message_js_1.createBuildStatus)(common_enum_js_1.BuildStatus.REJECT, -1, null !== (n = e.stack) && undefined !== n ? n : e.message));
      }
      if ((0, hvigor_log_js_1.evaluateLogLevel)(global_core_parameters_js_1.coreParameter.startParams.logLevel, ["daemon"]), session_manager_js_1.defaultSessionManager.getActiveSocket() && (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() !== daemon_info_js_1.DaemonState.HALF_BUSY || (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() === daemon_info_js_1.DaemonState.STOP_REQ || (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() === daemon_info_js_1.DaemonState.BUSY) {
        const e = "hvigor daemon: Current process status is busy, cannot start a build action. Please wait a moment and retry.";
        return this._log.debug(`hvigor daemon: Build rejected - process busy. socketId=${null === (_ = this.socket) || undefined === _ ? undefined : _.id}, currentState=${(0, daemon_process_lifecycle_js_1.getCurrentProcessState)()}`), this._log.error(e), void this.socket.emit(build_event_js_1.BuildEvent.BUILD_STATUS, (0, message_js_1.createBuildStatus)(common_enum_js_1.BuildStatus.REJECT, 0, e));
      }
      session_manager_js_1.defaultSessionManager.setActiveSocket(this.socket), (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() === daemon_info_js_1.DaemonState.HALF_BUSY && (0, daemon_process_lifecycle_js_1.setCurrentProcessState)(daemon_info_js_1.DaemonState.STOP_REQ), this._log.debug(`hvigor daemon: Build start. socketId=${null === (a = this.socket) || undefined === a ? undefined : a.id}, currentState=${(0, daemon_process_lifecycle_js_1.getCurrentProcessState)()}`), (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() === daemon_info_js_1.DaemonState.STOP_REQ && (0, daemon_process_lifecycle_js_1.setCurrentProcessState)(daemon_info_js_1.DaemonState.BUSY);
      const {
        exitCode: m,
        reason: g
      } = await this.awaitBuildResult(c);
      (0, daemon_process_lifecycle_js_1.resetProcessTimeOutTimer)(e), (0, daemon_process_lifecycle_js_1.getCurrentProcessState)() === daemon_info_js_1.DaemonState.BUSY && (0, daemon_process_lifecycle_js_1.setCurrentProcessState)(daemon_info_js_1.DaemonState.HALF_BUSY), this._log.debug(`hvigor daemon: Build end. socketId=${null === (d = this.socket) || undefined === d ? undefined : d.id}, exitCode=${m}, duration=${(0, time_util_js_1.formatTime)(process_1.hrtime(l))}`), this.clearCache(), this.socket.emit(build_event_js_1.BuildEvent.BUILD_STATUS, (0, message_js_1.createBuildStatus)(common_enum_js_1.BuildStatus.FINISH, m, g));
    });
  }
  async getWorker() {
    var e, s;
    let o = Object.values(cluster_1.workers)[0];
    return o && o.isConnected() && !o.isDead() ? this._log.debug(`hvigor daemon: Reusing existing worker. \n      workerId=${null == o ? undefined : o.id}, pid=${null === (s = null == o ? undefined : o.process) || undefined === s ? undefined : s.pid}, isConnected=${null == o ? undefined : o.isConnected()}, isDead=${null == o ? undefined : o.isDead()}`) : (this._log.debug(`hvigor daemon: No available worker, forking new worker process. isConnected=${null == o ? undefined : o.isConnected()}, isDead=${null == o ? undefined : o.isDead()}`), o = cluster_1.fork(), this._log.debug("hvigor daemon: New worker forked."), o.process.stdout.pipe(process_1.stdout), o.process.stderr.pipe(process_1.stderr), await new Promise(e => {
      null == o || o.once("online", e);
    }), this._log.debug(`hvigor daemon: Worker is online. workerId=${null == o ? undefined : o.id}, pid=${null === (e = null == o ? undefined : o.process) || undefined === e ? undefined : e.pid}`)), o;
  }
  async awaitBuildResult(e) {
    let [s, o] = [0, undefined];
    if (!cluster_1.isPrimary) return {
      exitCode: s,
      reason: o
    };
    const t = await this.getWorker();
    return await new Promise(i => {
      session_manager_js_1.defaultSessionManager.bindProcess(this.socket.id, t.id);
      const r = e => {
          if (e.type) {
            if (e.type === worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FINISHED) s = 0, this._log.debug(`hvigor daemon: worker process ${t.process.pid} finished. Build Finished.`);else {
              if (e.type !== worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FAILED) return;
              s = -1, this._log.debug(`hvigor daemon: worker process ${t.process.pid} finished. Build Failed.`), o = e.content;
            }
            t.off("exit", n), t.off("message", r), i();
          }
        },
        n = (e, o) => {
          this._log.debug(`hvigor daemon: worker process ${t.process.pid} exit (${e || o}). Build Failed.`), s = e, t.off("message", r), i();
        };
      t.on("message", r), t.once("exit", n), t.send({
        type: build_event_js_1.BuildEvent.COMMON_BUILD,
        content: e
      });
    }), {
      exitCode: s,
      reason: o
    };
  }
  onCancel() {
    this.socket.on(build_event_js_1.BuildEvent.CANCEL_BUILD, e => {
      if (!e) return void this.handleClientMsgError(e);
      const s = e;
      this._log.info(util_1.format(s)), this.stopTask(), this.clearCache(), this.socket.emit(common_enum_js_1.BuildStatus.CLOSE, (0, message_js_1.createBuildStatus)(common_enum_js_1.BuildStatus.CLOSE, 0)), session_manager_js_1.defaultSessionManager.resetActiveSocket();
    });
  }
  onStop() {
    this.socket.on(build_event_js_1.BuildEvent.STOP_DAEMON, e => {
      if (!e) return void this.handleClientMsgError(e);
      const s = e;
      this._log.info(util_1.format(s)), this.clearCache(), process_1.exit();
    });
  }
  onError() {
    this.socket.on("error", e => {
      this._log.error("error", e), this.stopTask(), this.clearCache();
    });
  }
  onDisconnected(e) {
    this.socket.on("disconnect", s => {
      this._log.debug("hvigor disconnect: ", s), this.stopTask(), this.clearCache(), e.emit(hvigor_daemon_server_js_1.DaemonServerEventName.CLOSE_CONNECTION, this.socket);
    });
  }
  stopTask() {
    var e;
    (null === (e = session_manager_js_1.defaultSessionManager.getActiveSocket()) || undefined === e ? undefined : e.id) === this.socket.id && session_manager_js_1.defaultSessionManager.postMessageToThread(this.socket.id, {
      type: event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED,
      reason: "Manually stop by user."
    }), session_manager_js_1.defaultSessionManager.postMessageToThread(this.socket.id, {
      type: build_event_js_1.WatchEvent.CLOSE_WATCH
    });
  }
  clearCache() {
    hvigor_config_loader_js_1.HvigorConfigLoader.clean(), hvigor_log_js_1.HvigorLogger.clean();
  }
  handleClientMsgError(e) {
    this._log.warn("Received client exception type information."), this._log.warn(`receive client message: ${util_1.format(e)}`);
  }
}
