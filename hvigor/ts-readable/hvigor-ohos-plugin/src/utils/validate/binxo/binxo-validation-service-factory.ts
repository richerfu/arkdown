/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/binxo/binxo-validation-service-factory.js
 */
import * as binxo_validation_service_js_1 from "./binxo-validation-service";
export class BinxoValidationServiceFactory {
  static createBinxoValidationService(i) {
    return new binxo_validation_service_js_1.BinxoValidationService(i);
  }
}
