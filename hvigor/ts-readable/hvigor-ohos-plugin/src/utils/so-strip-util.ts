/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/so-strip-util.js
 */
import * as glob_1 from "glob";
import * as common_const_js_1 from "../const/common-const";
export class SoStripUtil {
  static matchedFiles(t, o, e) {
    const s = new Set();
    return o ? (o.forEach(o => {
      glob_1.glob.sync(o, {
        cwd: e
      }).forEach(o => {
        if (t) s.add(o);else {
          new RegExp(common_const_js_1.NativeConst.LIBRARY_FILE_REGEXP).test(o) && s.add(o);
        }
      });
    }), Array.from(s)) : Array.from(s);
  }
  static filesExcluding(t, o, e) {
    return glob_1.glob.sync(true === t ? common_const_js_1.NativeConst.LIBRARY_ALL_FILE_PATTERN : common_const_js_1.NativeConst.LIBRARY_FILE_PATTERN, {
      cwd: e,
      ignore: o
    });
  }
  static needStrip(t) {
    return !t || false !== t.strip || !!(t.exclude && t.exclude.length > 0);
  }
}
