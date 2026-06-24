/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/server/hvigor-daemon-server.js
 */
import events_1 from "events";
import * as http from "http";
import * as socket_io_1 from "socket.io";
export var DaemonServerEventName;
!function (e) {
  e.CONNECTION = "connection", e.LISTENING = "listening", e.LISTEN_FAILED = "listenFailed", e.CLOSE_CONNECTION = "closeConnection", e.CLOSE_SERVER = "closeServer", e.ERROR = "error";
}(DaemonServerEventName = DaemonServerEventName || (DaemonServerEventName = {}));
export class HvigorDaemonServer extends events_1 {
  constructor(e) {
    super(), this.httpServer = http.createServer(), this.server = new socket_io_1.Server(this.httpServer, {
      path: `/${e}`,
      pingInterval: 6e4,
      pingTimeout: 6e4
    }), this.sessionId = e;
  }
  static getInstance(e) {
    return HvigorDaemonServer.instance || (HvigorDaemonServer.instance = new HvigorDaemonServer(e)), HvigorDaemonServer.instance;
  }
  getHttpServer() {
    return this.httpServer;
  }
  getSessionId() {
    return this.sessionId;
  }
  onConnection(e) {
    this.server.on(DaemonServerEventName.CONNECTION, e);
  }
  onError(e) {
    this.server.on(DaemonServerEventName.ERROR, e);
  }
  listen(e, t, r) {
    this.httpServer.listen(e, t, r);
  }
  onListening(e) {
    this.httpServer.on(DaemonServerEventName.LISTENING, e);
  }
  onListenFailed(e) {
    this.on(DaemonServerEventName.LISTEN_FAILED, e);
  }
  onCloseConnection(e) {
    this.on(DaemonServerEventName.CLOSE_CONNECTION, e);
  }
  onCloseServer(e) {
    this.on(DaemonServerEventName.CLOSE_SERVER, e);
  }
}
