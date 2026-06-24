/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/fs/default-fs-snapshot-generator.js
 */
import * as json_reader_js_1 from "../../../util/json-reader";
import * as filesystem_snapshot_js_1 from "../core/filesystem-snapshot";
export class DefaultFsSnapshotGenerator {
  snapshot(e, s) {
    return 0;
  }
  loadSnapshotCacheFromJson(e) {
    const s = json_reader_js_1.JsonReader.parseJsonText(e, new Map()),
      t = new Map();
    for (const [e, o] of Object.entries(s)) t.set(e, Object.setPrototypeOf(o, filesystem_snapshot_js_1.BasicFileSnapshot.prototype));
    return t;
  }
  loadSnapshotFromJson(e) {
    const s = json_reader_js_1.JsonReader.parseJsonText(e, {});
    return Object.setPrototypeOf(s, filesystem_snapshot_js_1.BasicFileSnapshot), s;
  }
  serializeSnapshotToJson(e) {
    const s = Object.fromEntries(e.entries());
    return JSON.stringify(s);
  }
}
