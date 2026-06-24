/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/hvigor-user-home.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as path_const_js_1 from "../const/path-const";
let hvigorUserHomeCacheDirHasLogged = false;
export function getHvigorUserHomeCacheDir() {
  const e = path_const_js_1.DEFAULT_HVIGOR_USER_HOME_CACHE_DIR,
    t = process.env.HVIGOR_USER_HOME;
  return undefined === t ? e : path_1.isAbsolute(t) ? fs_extra_1.existsSync(t) && fs_extra_1.statSync(t).isFile() ? (console.log(`File already exists: ${t}`), e) : (fs_extra_1.ensureDirSync(t), t) : (hvigorUserHomeCacheDirHasLogged || (console.log(`Invalid custom userhome hvigor data dir:${t}`), hvigorUserHomeCacheDirHasLogged = true), e);
}
