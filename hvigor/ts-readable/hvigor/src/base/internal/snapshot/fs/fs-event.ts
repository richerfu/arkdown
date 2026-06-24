/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/fs/fs-event.js
 */
export class FsEvent {
  constructor(t, e, s, r) {
    this.eventType = t, this.name = e, this.file = s, this.stats = r;
  }
  getName() {
    return this.name;
  }
  type() {
    return this.eventType;
  }
  getFile() {
    return this.file;
  }
  getStats() {
    return this.stats;
  }
}
