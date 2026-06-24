/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/arkui-x/arkui-x-project-model.js
 */
import path_1 from "path";
import * as fse from "fs-extra";
import * as validate_util_js_1 from "../../../utils/validate/validate-util";
import * as common_const_js_1 from "../../../const/common-const";
import * as json_util_js_1 from "../../../utils/json-util";
import * as crossplatform_const_js_1 from "../../../const/crossplatform-const";
import * as ohos_logger_js_1 from "../../../utils/log/ohos-logger";
export class ArkUIXProjectModel {
  constructor(t, e, o) {
    this.projectName = t, this.projectPath = e, this.profileOptions = o, this._log = ohos_logger_js_1.OhosLogger.getLogger(ArkUIXProjectModel.name), this.initArkUIXConfigJsonObj();
  }
  isCrossplatformProject() {
    return !!this._arkUIXConfigJsonObj && this._arkUIXConfigJsonObj.crossplatform;
  }
  getArkUIXConfigJsonPath() {
    return path_1.resolve(this.projectPath, crossplatform_const_js_1.ARKUIX_ROOT, crossplatform_const_js_1.ARKUIX_CONFIG_JSON5);
  }
  getArkUIXConfigJsonObj() {
    return this._arkUIXConfigJsonObj;
  }
  initArkUIXConfigJsonObj() {
    const t = this.getArkUIXConfigJsonPath();
    fse.existsSync(t) && (validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.projectName,
      filePath: t,
      schemaPath: common_const_js_1.BuildProfileSchemaFileConst.ARKUI_X_CONFIG_SCHEMA_PATH
    }), this._arkUIXConfigJsonObj = json_util_js_1.JsonUtil.getJson5Obj(t), this.arkUIXModuleCheck());
  }
  arkUIXModuleCheck() {
    var t;
    const e = null === (t = this._arkUIXConfigJsonObj) || undefined === t ? undefined : t.modules.reduce((t, e) => (this.profileOptions.modules.some(t => t.name === e) || t.push(e), t), []);
    e && e.length && this._log.printErrorExit("INVALID_ARKUI_X_MODULE_NAME", [e.join('","'), this.getArkUIXConfigJsonPath()], [[this.getArkUIXConfigJsonPath()]]);
  }
}
