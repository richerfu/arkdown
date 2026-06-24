/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/core/filesystem-snapshot.js
 */
import * as file_type_js_1 from "../util/file-type";
import * as snapshot_js_1 from "./snapshot";
export class BasicFileSnapshot extends snapshot_js_1.Snapshot {
  constructor(s = "", t = "", e = file_type_js_1.FileType.UNKNOWN, i = false) {
    super(), this.hashValue = "", this.name = s, this.path = t, this.type = e, this.isSymbolicLink = i;
  }
  isHashEquals(s) {
    return this.hashValue === s.hashValue;
  }
  toJsonString() {
    return JSON.stringify(this);
  }
  [Symbol.iterator]() {
    return this.walk();
  }
}
