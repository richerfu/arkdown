/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/legacy-dependency-manager-helper.js
 */
import fs_1 from "fs";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as error_url_js_1 from "../../utils/error-url";
import * as common_const_js_1 from "../../const/common-const";
import path_1 from "path";
const log = ohos_logger_js_1.OhosLogger.getLogger("ohos-legacy-dependency-manager-helper"),
  solution = `The OpenHarmony npm package or module is not compatible with the current model. Use a compatible OpenHarmony npm package or module. For details about the stage and FA models, see ${error_url_js_1.ErrorUrl.ERROR_URL.modelDiff}.`;
export class LegacyDependencyManagerHelper {
  constructor(e) {
    this._moduleName = e;
  }
  checkHspOrHarStatusForFAByProfile(e, o) {
    const r = path_1.resolve(e, "src", "main", common_const_js_1.CommonConst.MODULE_JSON5),
      t = path_1.resolve(e, "src", "main", common_const_js_1.CommonConst.MODULE_JSON);
    return this.checkHspOrHarStatusForFA(r, o) || this.checkHspOrHarStatusForFA(t, o);
  }
  checkHspOrHarStatusForFA(e, o) {
    return (null == o ? undefined : o.name) === common_const_js_1.CommonConst.HYPIUM_PACKAGE_NAME || fs_1.existsSync(e) && log.printErrorExit("FA_RELY_OH_STAGE", [this._moduleName, e], [[solution]]), true;
  }
}
