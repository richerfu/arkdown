/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/session/session-map.js
 */
import * as bidirectional_map_js_1 from "../util/bidirectional-map";
export class SessionMap {
  constructor() {
    this.sessionMap = new bidirectional_map_js_1.BidirectionalMap();
  }
  getSocketId(e) {
    return this.sessionMap.getByValue(e);
  }
  getThreadId(e) {
    return this.sessionMap.getByKey(e);
  }
  setSession(e, s) {
    this.sessionMap.checkByKey(e) && this.sessionMap.deleteByKey(e), this.sessionMap.checkByValue(s) && this.sessionMap.deleteByValue(s), this.sessionMap.set([e, s]);
  }
  deleteByThreadId(e) {
    this.sessionMap.deleteByValue(e);
  }
  deleteBySocketId(e) {
    this.sessionMap.deleteByKey(e);
  }
  checkBySocketId(e) {
    return this.sessionMap.checkByKey(e);
  }
  checkByThreadId(e) {
    return this.sessionMap.checkByValue(e);
  }
}
export function getThreadId(e, s) {
  return `${e}@${s}`;
}
export function resolveThreadId(e) {
  return e.split("@").map(e => parseInt(e));
}
