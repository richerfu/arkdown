/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/comparator/filesystem-snapshot-detail-comparator.js
 */
export class FileSystemSnapshotNameComparator {
  compare(a, e) {
    return a.name.localeCompare(e.name);
  }
}
export class FileSystemSnapshotHashValueComparator {
  compare(a, e) {
    return a.hashValue.localeCompare(e.hashValue);
  }
}
