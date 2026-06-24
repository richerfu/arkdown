/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/log-collection/log-collector.js
 */
export class LogCollector {
  constructor() {
    this.logSet = new Set();
  }
  add(e) {
    this.logSet.add(e);
  }
  dump() {
    return this.logSet;
  }
  clear() {
    this.logSet.clear();
  }
}
