/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/meta-util.js
 */
import fs_1 from "fs";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as path_util_js_1 from "../../common/util/path-util";
import * as hvigor_log_js_1 from "../log/hvigor-log";
import * as json_reader_js_1 from "./json-reader";
const logger = hvigor_log_js_1.HvigorLogger.getLogger("meta-util"),
  hvigorVersion = "hvigorVersion";
function getMetaCacheFilePath() {
  return path_1.resolve(path_util_js_1.PathUtil.getHvigorCacheDir(logger), "./cache/meta.json");
}
export function getMetaProperty(e) {
  const t = getMetaCacheFilePath();
  return fs_1.existsSync(t) ? json_reader_js_1.JsonReader.getJsonObj(t)[e] : undefined;
}
export function updateMetaProperties(e) {
  if (!e.size) return;
  const t = getMetaCacheFilePath(),
    r = fs_1.existsSync(t) ? json_reader_js_1.JsonReader.getJsonObj(t) : {};
  e.forEach((e, t) => r[t] = e), fs_extra_1.ensureFileSync(t), fs_extra_1.writeJSONSync(t, r);
}
export function getLastHvigorVersion() {
  return getMetaProperty(hvigorVersion);
}
