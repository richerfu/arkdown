/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/generator/default-filesystem-snapshot-generator.js
 */
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as directory_snapshot_js_1 from "../core/directory-snapshot";
import * as file_snapshot_js_1 from "../core/file-snapshot";
import * as file_hasher_js_1 from "../util/file-hasher";
import * as file_metadata_js_1 from "../util/file-metadata";
import * as file_tree_js_1 from "../util/file-tree";
import * as file_type_js_1 from "../util/file-type";
class FileVisitorImpl {
  constructor(e) {
    this.rootSnapShot = undefined, this.stack = [], this.fileHasher = e;
  }
  preVisitDirectory(e) {
    const t = new directory_snapshot_js_1.DirectorySnapshot(e.name, e.path, e.type, e.isSymbolicLink);
    if (undefined === this.rootSnapShot && (this.rootSnapShot = t), this.stack.length > 0) {
      this.stack[this.stack.length - 1].children.push(t);
    }
    this.stack.push(t);
  }
  visitFile(e) {
    const t = new file_metadata_js_1.FileMetaData(e.size, e.lastModifiedTime),
      s = new file_snapshot_js_1.FileSnapshot(e.name, e.path, e.type, e.isSymbolicLink, t);
    if (s.hashValue = this.fileHasher.hash(e.path, t), undefined === this.rootSnapShot && (this.rootSnapShot = s), this.stack.length > 0) {
      this.stack[this.stack.length - 1].children.push(s);
    }
  }
  postVisitDirectory(e) {
    const t = this.stack.pop();
    if (t && t.children.length > 0) {
      t.children.sort((e, t) => e.name.localeCompare(t.name));
      let e = "";
      t.children.forEach(t => {
        e += t.name + t.hashValue;
      }), t.hashValue = (0, hvigor_common_1.hash)(e);
    }
  }
  visitFileFailed(e, t) {}
}
export class DefaultFilesystemSnapshotGenerator {
  constructor(e = new file_hasher_js_1.FileMetaHasher()) {
    this.fileHasher = e;
  }
  parseFileSystemSnapshot(e) {
    var t, s, i, o, a, r, l;
    const n = e.type,
      h = null !== (t = e.hashValue) && undefined !== t ? t : "",
      p = null !== (s = e.name) && undefined !== s ? s : "",
      c = null !== (i = e.path) && undefined !== i ? i : "",
      _ = null !== (o = e.isSymbolicLink) && undefined !== o ? o : "";
    if (n === file_type_js_1.FileType.FILE.toString()) {
      const t = new file_metadata_js_1.FileMetaData(),
        s = e.fileMetaData;
      if (s) {
        const e = null !== (a = s.size) && undefined !== a ? a : 0,
          i = null !== (r = s.lastModifiedTime) && undefined !== r ? r : 0;
        t.size = e, t.lastModifiedTime = i;
      }
      const i = new file_snapshot_js_1.FileSnapshot(p, c, file_type_js_1.FileType.FILE, _, t);
      return i.hashValue = h, i;
    }
    if (n === file_type_js_1.FileType.DIRECTORY.toString()) {
      const t = new directory_snapshot_js_1.DirectorySnapshot(p, c, file_type_js_1.FileType.DIRECTORY, _);
      t.hashValue = h;
      const s = null !== (l = e.children) && undefined !== l ? l : [];
      for (const e of s) {
        const s = this.parseFileSystemSnapshot(e);
        undefined !== s && t.children.push(s);
      }
      return t;
    }
  }
  snapshot(e, t) {
    var s;
    const i = new FileVisitorImpl(this.fileHasher),
      o = {};
    return t && t.test && (o.filter = t.test), (0, file_tree_js_1.walkFileTree)(e, i, o), null !== (s = i.rootSnapShot) && undefined !== s ? s : new directory_snapshot_js_1.DirectorySnapshot("emptyroot", "", file_type_js_1.FileType.UNKNOWN, false);
  }
  loadSnapshotFromJson(e) {
    let t;
    try {
      t = JSON.parse(e);
    } catch (e) {
      return;
    }
    return this.parseFileSystemSnapshot(t);
  }
  loadSnapshotCacheFromJson(e) {
    let t;
    try {
      t = JSON.parse(e);
    } catch (e) {
      return;
    }
    const s = new Map();
    return Object.keys(t).forEach(e => {
      const i = this.parseFileSystemSnapshot(t[e]);
      undefined !== i && s.set(e, i);
    }), s;
  }
  serializeSnapshotToJson(e) {
    let t = "{";
    return e.forEach((e, s) => {
      t += JSON.stringify(s) + ":" + JSON.stringify(e) + ",";
    }), t.length > 1 && (t = t.slice(0, t.length - 1)), t += "}", t;
  }
}
