/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/session/session-manager.js
 */
import cluster_1 from "cluster";
import events_1 from "events";
import * as build_event_js_1 from "../../common/daemon-protocol/build-event";
import * as message_js_1 from "../../common/daemon-protocol/message";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as session_map_js_1 from "./session-map";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("daemon");
export const SESSION_EMIT_LOG = "emitLog";
class SessionManager extends events_1 {
  constructor() {
    super(), this.sessionMap = new session_map_js_1.SessionMap(), this.serverSockets = [], this.addListenersOnSocket();
  }
  initSession(e) {
    this.serverSockets.push(e), this.sessionMap.setSession(e.id, undefined);
  }
  endSession(e) {
    const s = this.serverSockets.indexOf(e);
    -1 !== s && (this.serverSockets.splice(s, 1), this.sessionMap.deleteBySocketId(e.id));
  }
  bindThread(e, s) {
    this.sessionMap.checkBySocketId(e) && this.sessionMap.setSession(e, s);
  }
  bindProcess(e, s) {
    this.sessionMap.checkBySocketId(e) && this.sessionMap.setSession(e, (0, session_map_js_1.getThreadId)(s, 0));
  }
  getSockets() {
    return this.serverSockets;
  }
  getLength() {
    return this.serverSockets.length;
  }
  getSocketByThreadId(e) {
    const s = this.sessionMap.getSocketId(e);
    return this.serverSockets.find(e => e.id === s);
  }
  getSocketBySocketId(e) {
    return this.serverSockets.find(s => s.id === e);
  }
  hasSocket(e) {
    return this.serverSockets.some(s => s.id === e);
  }
  getThreadId(e) {
    return this.sessionMap.getThreadId(e);
  }
  postMessageToThread(e, s) {
    const t = this.getThreadId(e);
    if (!t) return void _log.warn(`session manager: cannot find corresponding thread for socket id ${e}.`);
    const [o, i] = (0, session_map_js_1.resolveThreadId)(t);
    if (cluster_1.isMaster) {
      const e = cluster_1.workers[o];
      if (undefined === e) return void _log.warn(`session manager: cannot find corresponding worker process for process id ${o}.`);
      _log.debug("session manager: send message to worker process."), e.send({
        ...s,
        workerId: i
      });
    } else _log.warn("session manager: not master process, should be an error.");
  }
  setActiveSocket(e) {
    _log.debug(`session manager: set active socket. socketId=${e.id}`), this.curActiveSocket = e;
  }
  resetActiveSocket() {
    this.curActiveSocket = undefined;
  }
  getActiveSocket() {
    return this.curActiveSocket;
  }
  addListenersOnSocket() {
    cluster_1.isPrimary && cluster_1.on("message", (e, s) => {
      s.type && this.addListeners(s, e);
    });
  }
  addListeners(e, s) {
    let t;
    switch (e.type) {
      case build_event_js_1.WatchEvent.NEW_WATCH_WORKER:
        if (this.curActiveSocket) {
          const t = this.curActiveSocket.id,
            o = (0, session_map_js_1.getThreadId)(s.id, e.workerId);
          this.bindThread(t, o), _log.debug(`session manager: binding session. socketId=${t}, threadId=${o}.`), this.curActiveSocket.on(build_event_js_1.WatchEvent.WATCH_COMPILE_DATA, async e => {
            _log.debug("session manager: receive watch compile data."), defaultSessionManager.postMessageToThread(t, {
              type: build_event_js_1.WatchEvent.WATCH_COMPILE_DATA,
              content: e
            });
          }), this.curActiveSocket.on(build_event_js_1.WatchEvent.SEND_KEEP_ALIVE_SIGNAL, async () => {
            _log.debug("session manager: receive keep alive signal request."), defaultSessionManager.postMessageToThread(t, {
              type: build_event_js_1.WatchEvent.SEND_KEEP_ALIVE_SIGNAL
            });
          }), this.curActiveSocket.on(build_event_js_1.WatchEvent.CLOSE_WATCH, async () => {
            _log.debug("session manager: receive close watch request."), defaultSessionManager.postMessageToThread(t, {
              type: build_event_js_1.WatchEvent.CLOSE_WATCH
            });
          });
        }
        break;
      case build_event_js_1.WatchEvent.WATCH_COMPILE_RESULT:
      case build_event_js_1.WatchEvent.WATCH_START:
      case build_event_js_1.WatchEvent.WATCH_RESULT:
      case build_event_js_1.WatchEvent.WATCH_LOG:
        t = this.getSocketByThreadId((0, session_map_js_1.getThreadId)(s.id, e.workerId)), _log.debug(`session manager: watch message received from worker ${s.id}. Type: ${e.type}`), undefined === t || t.disconnected ? _log.debug(`session manager: Invalid socket, socket is undefined or disconnected. Type: ${e.type}`) : t.emit(e.type, e.content);
      case build_event_js_1.WatchEvent.TERMINATE_WORKER:
    }
  }
}
export const defaultSessionManager = new SessionManager();
defaultSessionManager.on(SESSION_EMIT_LOG, (e, s, t) => {
  const o = defaultSessionManager.getActiveSocket();
  undefined !== o && o.emit(build_event_js_1.BuildEvent.OUTPUT, (0, message_js_1.createOutput)(e, s, t));
});
