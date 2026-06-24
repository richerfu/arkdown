/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/common-sign-command-builder.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as build_option_path_info_js_1 from "../../../common/build-option-path-info";
import * as decipher_util_js_1 from "../../../utils/decipher-util";
import * as ohos_logger_js_1 from "../../../utils/log/ohos-logger";
import * as sdk_version_js_1 from "../../../version/sdk-version";
export class CommonSignCommandBuilder {
  constructor(i, t, e, o, s) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(CommonSignCommandBuilder.name), this._keyStorePwd = "", this._keyPwd = "", this._projectModel = i, this._signingConfig = t, this._sdkInfo = e, this._signModel = o, this._signingOptions = s;
    const n = this._sdkInfo.getToolchainsComponentVersion();
    undefined !== n && (this._toolchainsComponentVersion = new sdk_version_js_1.SdkVersion(n));
  }
  getSignCommand() {
    const i = this._signingConfig.material;
    return this.checkValidMaterial(i), this.initCommandParams(i), this._signingOptions.addCalledJarFile(this.getSignTool()), this._signingOptions.build();
  }
  getKeyStorePwd() {
    const i = build_option_path_info_js_1.buildOptionPath.getSignConfigPath(this._projectModel, "storePassword");
    return this._keyStorePwd = decipher_util_js_1.DecipherUtil.decryptPwd(path_1.resolve(this._signingConfig.material.storeFile, ".."), this._signingConfig.material.storePassword, i), this._keyStorePwd;
  }
  readKeyStorePwd() {
    return this._keyStorePwd;
  }
  getKeyPwd() {
    const i = build_option_path_info_js_1.buildOptionPath.getSignConfigPath(this._projectModel, "keyPassword");
    return this._keyPwd = decipher_util_js_1.DecipherUtil.decryptPwd(path_1.resolve(this._signingConfig.material.storeFile, ".."), this._signingConfig.material.keyPassword, i), this._keyPwd;
  }
  readKeyPwd() {
    return this._keyPwd;
  }
  checkValidMaterial(i) {
    undefined === i && this._log.printErrorExit("THE_MATERIAL_HAS_NOT_BEEN_CONFIGURED_IN_SIGNINGCONFIGS", [build_option_path_info_js_1.buildOptionPath.getSignConfigPath(this._projectModel, "certpath")]), this.validateMaterial(i.storeFile, "storeFile"), this.validateMaterial(i.profile, "profile"), this.validateMaterial(i.certpath, "certpath"), i.storeFile = this.normalizePath(i.storeFile), i.profile = this.normalizePath(i.profile), i.certpath = this.normalizePath(i.certpath);
  }
  normalizePath(i) {
    return path_1.isAbsolute(i) ? i : path_1.resolve(this._projectModel.getProjectDir(), i);
  }
  validateMaterial(i, t) {
    undefined !== i && fs_1.existsSync(this.normalizePath(i)) || this._log.printErrorExit("INVALID_%S_VALUE_MAKE_SURE_IT_IS_NOT_NULL_OR_EMPTY", [t, i, build_option_path_info_js_1.buildOptionPath.getSignConfigPath(this._projectModel, t)]);
  }
  validateSignType(i) {
    undefined === this._signingConfig.type && (this._signingConfig.type = "OpenHarmony"), this._signingConfig.type !== i && this._log.printErrorExit("SIGNING_CONFIGURATION_%S_DOES_NOT_APP_TO_%S", [this._signingConfig.name, i, build_option_path_info_js_1.buildOptionPath.getSignConfigPath(this._projectModel, "type")]);
  }
}
