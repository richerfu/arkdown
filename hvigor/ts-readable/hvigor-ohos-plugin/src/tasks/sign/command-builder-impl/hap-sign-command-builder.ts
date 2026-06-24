/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/hap-sign-command-builder.js
 */
import path_1 from "path";
import * as sdk_const_js_1 from "../../../const/sdk-const";
import * as common_sign_command_builder_js_1 from "./common-sign-command-builder";
export class HapSignCommandBuilder extends common_sign_command_builder_js_1.CommonSignCommandBuilder {
  constructor(e, i, t, d, o) {
    super(e, i, t, d, o);
  }
  getSignTool() {
    return path_1.resolve(this._sdkInfo.getSignDir(), sdk_const_js_1.ToolChainsConst.NEW_SIGN_TOOL);
  }
  initCommandParams(e) {
    this._signingOptions.sign().addMode(HapSignCommandBuilder.SIGN_MODE).addKeyStoreFile(e.storeFile).addKeyStorePwd(this.getKeyStorePwd()).addKeyAlias(e.keyAlias).addKeyPwd(this.getKeyPwd()).addSignAlg(e.signAlg).addProfileFile(e.profile).addAppCertFile(e.certpath).addInputFile(this._signModel.getInputFilePath()).addOutputFile(this._signModel.getOutPutFilePath());
  }
}
HapSignCommandBuilder.SIGN_MODE = "localSign";
