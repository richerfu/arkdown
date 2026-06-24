/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/pac-util.js
 */
import fs_extra_1 from "fs-extra";
import * as json_util_js_1 from "../../utils/json-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
const _log = ohos_logger_js_1.OhosLogger.getLogger("pac-util");
export function checkPacJsonRequiredFields(e) {
  var t;
  if (!fs_extra_1.existsSync(e)) return false;
  let s = {};
  try {
    s = null !== (t = (0, json_util_js_1.getJson5ObjThrowError)(e)) && undefined !== t ? t : {};
  } catch (e) {
    _log.debug("Catch error when parse pac.json5.", e);
  }
  const r = ["dataProcess", "specialAPIs"].some(e => Object.prototype.hasOwnProperty.call(s, e));
  return r || _log.warn(`'pac.json5' is not properly enabled. Please check if the file ${e} contains the required fields: 'dataProcess' or 'specialAPIs'.`), r;
}
export function checkPacJsonExistAndGetObj(e) {
  var t;
  if (fs_extra_1.existsSync(e)) try {
    return null !== (t = (0, json_util_js_1.getJson5ObjThrowError)(e)) && undefined !== t ? t : {};
  } catch (e) {
    _log.debug("Catch error when parse pac.json5.", e);
  }
}
export function pacFileValidate(e, t, s) {
  return !(!fs_extra_1.existsSync(t) || !fs_extra_1.existsSync(s)) && !!checkPacJsonRequiredFields(t) && (validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
    moduleName: e,
    filePath: t,
    schemaPath: s
  }), true);
}
