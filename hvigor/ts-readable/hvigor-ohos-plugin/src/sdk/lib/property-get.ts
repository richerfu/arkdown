/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/lib/property-get.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as property_const_js_1 from "../../const/property-const";
import * as file_util_js_1 from "../../utils/file-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
const localPropertiesPath = path_1.resolve(process.cwd(), common_const_js_1.CommonConst.LOCAL_PROPERTIES);
export class Property {
  constructor(t = true) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(Property.name), this._properties = new Map(), this._isHarmonyOS = t, this._readFile().split(/\r?\n/).forEach(this._setProperty, this), this._properties.delete(property_const_js_1.PropertyConst.HWSDK_DIR), this._readPath();
  }
  _readFile() {
    return this._isHarmonyOS || !fs_extra_1.existsSync(localPropertiesPath) ? "" : fs_extra_1.readFileSync(localPropertiesPath, {
      encoding: "utf-8"
    });
  }
  getProperty(t) {
    if (this._properties.has(t)) {
      const r = this._properties.get(t);
      return r || "";
    }
    return "";
  }
  getSdkDir() {
    return this.validatePath(property_const_js_1.PropertyConst.SDK_DIR, property_const_js_1.PropertyConst.ENV_OHOS_BASE_SDK_HOME), this.getProperty(property_const_js_1.PropertyConst.SDK_DIR);
  }
  getHosSdkDir() {
    return this.validatePath(property_const_js_1.PropertyConst.HWSDK_DIR, property_const_js_1.PropertyConst.DEVECO_SDK_HOME), this.getProperty(property_const_js_1.PropertyConst.HWSDK_DIR);
  }
  getArkUIXSdkDir() {
    const t = this._properties.get(property_const_js_1.PropertyConst.ARKUIX_SDK_DIR);
    return t && fs_extra_1.existsSync(t) || this._log.printErrorExit("NO_ARKUI_X_SDK_DIR", [property_const_js_1.PropertyConst.ARKUIX_SDK_DIR, property_const_js_1.PropertyConst.ENV_ARKUIX_SDK_HOME, localPropertiesPath]), this.getProperty(property_const_js_1.PropertyConst.ARKUIX_SDK_DIR);
  }
  validatePath(t, r) {
    const e = this._properties.get(t);
    e || (this._isHarmonyOS ? this._log.printErrorExit("INVALID_VALUE_OF_%S_IN_THESYSTEM_ENVIRONMENT_PATH", [r]) : this._log.printErrorExit("NO_SDK_HOME", [t, r, localPropertiesPath], [[r]])), fs_extra_1.existsSync(path_1.resolve(e)) || (this._isHarmonyOS ? this._log.printErrorExit("INVALID_VALUE_OF_%S_IN_THESYSTEM_ENVIRONMENT_PATH", [r]) : this._log.printErrorExit("NO_SDK_HOME", [t, r, e], [[r]]));
  }
  _readPath() {
    this._setPathIfNotExist(property_const_js_1.PropertyConst.SDK_DIR, process.env[property_const_js_1.PropertyConst.ENV_OHOS_BASE_SDK_HOME]), this._setPathIfNotExist(property_const_js_1.PropertyConst.NPM_DIR, process.env[property_const_js_1.PropertyConst.ENV_NODE_HOME]), this._setPathIfNotExist(property_const_js_1.PropertyConst.ARKUIX_SDK_DIR, process.env[property_const_js_1.PropertyConst.ENV_ARKUIX_SDK_HOME]), this._setPathIfNotExist(property_const_js_1.PropertyConst.HWSDK_DIR, process.env[property_const_js_1.PropertyConst.DEVECO_SDK_HOME]);
  }
  _setPathIfNotExist(t, r) {
    !this._properties.has(t) && r && this._properties.set(t, file_util_js_1.FileUtil.normalizePathSeparator(r));
  }
  _setProperty(t) {
    if (!t || !t.trim()) return;
    const r = /^([^#=]+)(={0,1})(.*)$/.exec(t);
    r && this._properties.set(r[1].trim(), file_util_js_1.FileUtil.normalizePathSeparator(r[3]).trimStart());
  }
}
