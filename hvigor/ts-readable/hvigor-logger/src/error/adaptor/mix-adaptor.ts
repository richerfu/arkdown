/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/mix-adaptor.js
 */
import * as adaptor_factory_js_1 from "./adaptor-factory";
export class MixAdaptor {
  constructor(r) {
    this.adaptor = adaptor_factory_js_1.AdaptorFactory.createErrorAdaptor(r);
  }
  getErrorMessage() {
    return this.adaptor.getErrorMessage();
  }
}
