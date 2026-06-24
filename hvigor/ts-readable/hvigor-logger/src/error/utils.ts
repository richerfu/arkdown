/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/utils.js
 */
import path_1 from "path";
import * as errorlog_consts_js_1 from "../consts/errorlog-consts";
import * as error_util_js_1 from "../utils/error-util";
import * as hvigor_error_adaptor_js_1 from "./adaptor/hvigor-error-adaptor";
import * as constant_js_1 from "./constant";
import * as type_js_1 from "./type";
export function getErrorInfoFilePath(r) {
  var o;
  const t = null !== (o = constant_js_1.ErrorCodeToFile[r]) && undefined !== o ? o : constant_js_1.ErrorCodeToFile[type_js_1.ToolErrorCode.TEC_00];
  return path_1.resolve(constant_js_1.ERROR_INFO_DIRCTORY_PATH, t);
}
export function formatErrorAdaptorExport(r, o, t) {
  let e = new hvigor_error_adaptor_js_1.HvigorErrorAdaptor(r);
  return o && (e = e.formatMessage(...o)), t && t.forEach((r, o) => {
    e = e.formatSolutions(o, ...r);
  }), e;
}
export function errorCode2Description(r) {
  const o = error_util_js_1.ErrorUtil.getErrorTypeCodeFromErrorCode(r);
  return o ? errorlog_consts_js_1.ErrorCodeDescription[o] : "";
}
