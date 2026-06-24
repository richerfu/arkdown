/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/core/directory-snapshot.js
 */
import * as filesystem_snapshot_visitor_js_1 from "../generator/filesystem-snapshot-visitor";
import * as filesystem_snapshot_js_1 from "./filesystem-snapshot";
export class DirectorySnapshot extends filesystem_snapshot_js_1.BasicFileSnapshot {
  constructor() {
    super(...arguments), this.children = [];
  }
  accept(s) {
    switch (s.visitEntry(this)) {
      case filesystem_snapshot_visitor_js_1.SnapshotVisitResult.CONTINUE:
        s.enterDirectory(this);
        for (const t of this.children) t.accept(s);
        return s.leaveDirectory(this), filesystem_snapshot_visitor_js_1.SnapshotVisitResult.CONTINUE;
      case filesystem_snapshot_visitor_js_1.SnapshotVisitResult.SKIP_SUBTREE:
        return filesystem_snapshot_visitor_js_1.SnapshotVisitResult.CONTINUE;
      case filesystem_snapshot_visitor_js_1.SnapshotVisitResult.TERMINATE:
        return filesystem_snapshot_visitor_js_1.SnapshotVisitResult.TERMINATE;
      default:
        return filesystem_snapshot_visitor_js_1.SnapshotVisitResult.CONTINUE;
    }
  }
  *walk() {
    yield this;
    for (const s of this.children) yield* s.walk();
  }
  equals(s) {
    return s instanceof DirectorySnapshot && super.isHashEquals(s);
  }
}
