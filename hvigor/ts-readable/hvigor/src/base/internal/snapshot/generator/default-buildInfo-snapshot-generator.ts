/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/generator/default-buildInfo-snapshot-generator.js
 */
import * as snapshot_js_1 from "../core/snapshot";
import * as global_data_js_1 from "../../data/global-data";
import * as json_reader_js_1 from "../../../util/json-reader";
export class DefaultBuildInfoSnapshotGenerator {
  loadSnapshotCacheFromJson(o) {
    const a = json_reader_js_1.JsonReader.parseJsonText(o, snapshot_js_1.Snapshot),
      t = new Map();
    for (const [o, l] of Object.entries(a)) t.set(o, Object.setPrototypeOf(l, snapshot_js_1.Snapshot.prototype));
    return t;
  }
  serializeSnapshotCacheToJson() {
    var o, a, t, l, e;
    const s = {
      _: null === (o = global_data_js_1.globalData.cliOpts) || undefined === o ? undefined : o._
    };
    return 1 == (null === (a = global_data_js_1.globalData.cliOpts) || undefined === a ? undefined : a.hotReloadBuild) && (s.hotReloadBuild = null === (t = global_data_js_1.globalData.cliOpts) || undefined === t ? undefined : t.hotReloadBuild), 1 == (null === (l = global_data_js_1.globalData.cliOpts) || undefined === l ? undefined : l.hotCompile) && (s.hotCompile = null === (e = global_data_js_1.globalData.cliOpts) || undefined === e ? undefined : e.hotCompile), JSON.stringify(s);
  }
}
