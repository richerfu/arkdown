/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/adaptor-factory.js
 */
import * as constant_js_1 from "../constant";
import * as eight_digit_error_code_adaptor_js_1 from "./eight-digit-error-code-adaptor";
import * as hvigor_global_error_adaptor_js_1 from "./hvigor-global-error-adaptor";
import * as pack_tool_adaptor_js_1 from "./pack-tool-adaptor";
import * as third_party_error_adaptor_js_1 from "./third-party-error-adaptor";
export class AdaptorFactory {
  static createErrorAdaptor(r) {
    return AdaptorFactory.hasErrorCode(r) ? new eight_digit_error_code_adaptor_js_1.EightDigitErrorCodeAdaptor(r) : AdaptorFactory.isThirdPartyError(r) ? r.includes("BundleTool") ? new pack_tool_adaptor_js_1.PackToolAdaptor(r) : new third_party_error_adaptor_js_1.ThirdPartyErrorAdaptor(r) : new hvigor_global_error_adaptor_js_1.HvigorGlobalErrorAdaptor(r);
  }
  static isThirdPartyError(r) {
    return r.includes(third_party_error_adaptor_js_1.ThirdPartyErrorAdaptor.SOLUTIONS);
  }
  static hasErrorCode(r) {
    return new RegExp(constant_js_1.EIGHT_DIGIT_ERROR_REGEXP.PATTERN, constant_js_1.EIGHT_DIGIT_ERROR_REGEXP.FLAGS).test(r);
  }
}
