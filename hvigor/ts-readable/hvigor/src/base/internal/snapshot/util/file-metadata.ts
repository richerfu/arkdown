/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/file-metadata.js
 */
export class FileMetaData {
  constructor(e = 0, t = 0) {
    this.size = e, this.lastModifiedTime = t;
  }
  equals(e) {
    return this.size === e.size && this.lastModifiedTime === e.lastModifiedTime;
  }
}
