/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/wrapper/util.js
 */
import path_1 from "path";
import process_1 from "process";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as const_js_1 from "../../common/const/const";
import * as path_const_js_1 from "../../common/const/path-const";
const HVIGOR_PROJECT_CACHE_PATH_HEX_LENGTH = 32;
export function calProjectCacheDir() {
  const e = (0, hvigor_common_1.hash)(path_1.resolve(process_1.cwd())).slice(0, 32);
  return path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, const_js_1.PROJECT_CACHES, e, const_js_1.WORK_SPACE);
}
export function getHvigorProjectHome() {
  const e = calProjectCacheDir();
  return process_1.env.WORKSPACE_DIR = e, e;
}
export function getCurrentProjectCacheDirHash() {
  var e;
  return (0, hvigor_common_1.hash)(null !== (e = process_1.env.NODE_PATH) && undefined !== e ? e : "");
}
