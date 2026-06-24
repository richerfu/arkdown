/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/fs/generic-fs-snapshot-builder.js
 */
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as directory_snapshot_js_1 from "../core/directory-snapshot";
import * as file_snapshot_js_1 from "../core/file-snapshot";
import * as file_metadata_js_1 from "../util/file-metadata";
import * as file_type_js_1 from "../util/file-type";
export class GenericFsSnapshotBuilder {
  constructor() {
    this.directoryStack = [];
  }
  enterDirectory(e) {
    const t = this.assembleSnapshot(e, true);
    this.directoryStack.push(t);
  }
  visitEntry(e) {
    const t = this.assembleSnapshot(e, false);
    this.hashItem(t), this.handleEntry(t);
  }
  leaveDirectory(e) {
    const t = this.directoryStack.pop();
    t && (this.completeDirectory(t), this.handleEntry(t));
  }
  handleEntry(e) {
    const t = this.peekLast();
    t ? t.children.push(e) : this.directorySnapshot = e;
  }
  completeDirectory(e) {
    const t = (0, hvigor_common_1.createHash)().update(e.name);
    e.children.sort().forEach(e => t.update(e.hashValue).update(e.name)), e.hashValue = t.digest("hex");
  }
  hashItem(e) {
    return (0, hvigor_common_1.hash)(`${e.name}${e.type}${e.fileMetaData.lastModifiedTime}${e.fileMetaData.size}`);
  }
  getResult() {
    return this.directorySnapshot;
  }
  assembleSnapshot(e, t) {
    if (t) return new directory_snapshot_js_1.DirectorySnapshot(e.getName(), e.getFile(), file_type_js_1.FileType.DIRECTORY, e.getStats().isSymbolicLink());
    const s = new file_metadata_js_1.FileMetaData(e.getStats().size, e.getStats().mtimeMs);
    return new file_snapshot_js_1.FileSnapshot(e.getName(), e.getFile(), file_type_js_1.FileType.FILE, e.getStats().isSymbolicLink(), s);
  }
  peekLast() {
    const e = this.directoryStack.length;
    if (!(e < 1)) return this.directoryStack[e - 1];
  }
}
