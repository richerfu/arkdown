/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/service/snapshot-comparator-service.js
 */
import * as default_task_snapshot_js_1 from "../core/default-task-snapshot";
import * as filesystem_snapshot_js_1 from "../core/filesystem-snapshot";
export class SnapshotComparatorService {
  static getInstance() {
    return SnapshotComparatorService._instance || (SnapshotComparatorService._instance = new SnapshotComparatorService()), SnapshotComparatorService._instance;
  }
  static isDiff(s, a) {
    return !s || !a || (s instanceof filesystem_snapshot_js_1.BasicFileSnapshot && a instanceof filesystem_snapshot_js_1.BasicFileSnapshot ? !s.equals(a) : !(s instanceof default_task_snapshot_js_1.DefaultTaskSnapshot && a instanceof default_task_snapshot_js_1.DefaultTaskSnapshot) || !s.compareTo(a));
  }
}
SnapshotComparatorService._instance = null;
