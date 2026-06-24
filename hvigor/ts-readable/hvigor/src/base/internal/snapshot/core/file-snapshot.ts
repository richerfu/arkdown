/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/core/file-snapshot.js
 */
import * as filesystem_snapshot_js_1 from "./filesystem-snapshot";
export class FileSnapshot extends filesystem_snapshot_js_1.BasicFileSnapshot {
  constructor(s, e, t, i, a) {
    super(s, e, t, i), this.fileMetaData = a;
  }
  accept(s) {
    return s.visitEntry(this);
  }
  *walk() {
    yield this;
  }
  equals(s) {
    return s instanceof FileSnapshot && super.isHashEquals(s);
  }
}
