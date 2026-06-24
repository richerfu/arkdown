/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/arkTs-error-adaptor.js
 */
import * as type_js_1 from "../type";
import * as utils_js_1 from "../utils";
import * as third_party_error_adaptor_js_1 from "./third-party-error-adaptor";
export class ArkTsErrorAdaptor extends third_party_error_adaptor_js_1.ThirdPartyErrorAdaptor {
  constructor(r, t) {
    super(r, t);
  }
  getErrorCodeJsonPath() {
    return [(0, utils_js_1.getErrorInfoFilePath)(type_js_1.ToolErrorCode.TEC_21)];
  }
}
