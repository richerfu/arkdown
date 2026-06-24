/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/cangjie-register.js
 */
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class CangjieRegisterService {
  constructor() {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(CangjieRegisterService.name);
  }
  dealCangjieSchema() {
    let e;
    try {
      e = require("@ohos/cangjie-build-support/index"), null == e || e.handleCangjieSchema();
    } catch (e) {
      this._log.debug(`failed to load cangjie to dynamically generate schemas: ${e}`);
    }
    return e;
  }
}
