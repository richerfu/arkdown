/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/error-util.js
 */
import * as errorlog_consts_js_1 from "../consts/errorlog-consts";
import * as adaptor_error_js_1 from "../error/adaptor-error";
import * as hvigor_error_common_adapter_js_1 from "../logger/hvigor-error-common-adapter";
import * as hvigor_error_info_js_1 from "../logger/hvigor-error-info";
export class ErrorUtil {
  static getRealError(r) {
    return r instanceof hvigor_error_info_js_1.HvigorErrorInfo ? r : new hvigor_error_info_js_1.HvigorErrorInfo({
      code: (null == r ? undefined : r.code) || errorlog_consts_js_1.UNDEFINED_CODE,
      cause: (null == r ? undefined : r.cause) || errorlog_consts_js_1.UNDEFINED_CAUSE,
      description: (null == r ? undefined : r.description) || errorlog_consts_js_1.UNDEFINED_DESC,
      position: (null == r ? undefined : r.position) || errorlog_consts_js_1.UNDEFINED_POS,
      solutions: (null == r ? undefined : r.solutions) || [],
      moreInfo: null == r ? undefined : r.moreInfo
    });
  }
  static combinePhase(r) {
    if ("" === r.description) {
      const o = this.getErrorTypeCodeFromErrorCode(r.code);
      if (o) {
        const e = errorlog_consts_js_1.ErrorCodeDescription[o];
        r.description = null != e ? e : r.description;
      }
    }
    const o = ErrorUtil.getRealError(r);
    return new hvigor_error_common_adapter_js_1.HvigorErrorCommonAdapter().convertErrorToLogInfo(o).getMessage();
  }
  static getErrorTypeCodeFromErrorCode(r) {
    if (r && !(r.length < 5)) return r.slice(3, 5);
  }
  static getFirstErrorAdaptorMessage(r) {
    var o;
    return null !== (o = r[0]) && undefined !== o ? o : {
      message: ""
    };
  }
  static isAdaptorError(r) {
    return r.name === adaptor_error_js_1.AdaptorError.NAME || r instanceof adaptor_error_js_1.AdaptorError;
  }
}
