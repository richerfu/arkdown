/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/store/tcb-store.js
 */
export class TcbStore {
  static getTCB(t) {
    return TcbStore.tcbMap.get(t);
  }
  static clear() {
    TcbStore.tcbMap.clear();
  }
  static add(t) {
    TcbStore.tcbMap.set(t.getId(), t);
  }
}
TcbStore.tcbMap = new Map();
