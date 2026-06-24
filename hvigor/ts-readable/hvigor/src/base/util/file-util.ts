/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/file-util.js
 */
import * as fs from "fs";
import * as common_const_js_1 from "../common/options/common-const";
export function readFile(e) {
  return new Promise((t, i) => {
    fs.readFile(e, (e, r) => {
      e ? i(e) : t(r);
    });
  });
}
export function writeFile(e, t) {
  return new Promise((i, r) => {
    fs.writeFile(e, t, e => {
      e ? r(e) : i();
    });
  });
}
export function exitIfNotExists(e, t, i, r) {
  fs.existsSync(t) && fs.realpathSync.native(t) === t || e.printErrorExit("PATH_NOT_FOUND", [t], [[i, r]]);
}
export function findRealHvigorFilePath(e, t) {
  if (e) {
    for (const t of common_const_js_1.HvigorCommonConst.BUILD_FILE_NAME_SUFFIX) {
      const i = `${e}${t}`;
      if (fs.existsSync(i)) return i;
    }
    null == t || t.printErrorExit("HVIGOR_FILE_NOT_FOUND", [`${e}.ts`]);
  }
}
