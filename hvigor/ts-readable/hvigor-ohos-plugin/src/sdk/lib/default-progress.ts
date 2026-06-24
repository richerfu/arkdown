/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/lib/default-progress.js
 */
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
const LOGGER = ohos_logger_js_1.OhosLogger.getLogger("Sdk");
export class DefaultProgress {
  debug(e) {
    LOGGER.debug(e);
  }
  error(e) {
    LOGGER.error(e);
  }
  fatal(e) {
    LOGGER.debug(e);
  }
  info(e) {
    LOGGER.info(e);
  }
  warn(e) {
    LOGGER.warn(e);
  }
}
