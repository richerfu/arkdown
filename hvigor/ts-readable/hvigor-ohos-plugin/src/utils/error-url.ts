/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/error-url.js
 */
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as json_util_js_1 from "./json-util";
export class ErrorUrl {}
function getErrorUrl() {
  const r = json_util_js_1.JsonUtil.getJson5Obj(`${__dirname}/../../res/error/error.json`),
    o = (0, hvigor_common_1.getOsLanguage)();
  return Object.keys(r).reduce((e, s) => ({
    ...e,
    [s]: r[s][o]
  }), {});
}
ErrorUrl.ERROR_URL = getErrorUrl();
