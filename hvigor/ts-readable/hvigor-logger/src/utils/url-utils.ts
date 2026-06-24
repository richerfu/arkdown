/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/url-utils.js
 */
import fs_1 from "fs";
import path_1 from "path";
export const DEVELOPER_URL = "DEVELOPER_URL";
const CONFIG_JSON_PATH = path_1.resolve(__dirname, "../../res/config.json");
export function getUrl(t) {
  const e = getJsonObj(CONFIG_JSON_PATH);
  if (e && e[t]) return e[t];
}
function getJsonObj(t, e = "utf-8") {
  if (fs_1.existsSync(t)) try {
    const r = fs_1.readFileSync(t, {
      encoding: e
    });
    return JSON.parse(r);
  } catch (t) {
    return;
  }
}
