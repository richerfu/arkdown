/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/har-sign-command-builder.js
 */
import * as common_sign_command_builder_js_1 from "./common-sign-command-builder";
export class HarSignCommandBuilder extends common_sign_command_builder_js_1.CommonSignCommandBuilder {
  constructor(e, i, d, o, n) {
    super(e, i, d, o, n);
  }
  getSignTool() {
    return this._sdkInfo.getHosSignTool();
  }
  initCommandParams(e) {
    this._signingOptions.addKeyStoreFile(e.storeFile).addKeyStorePwd(this.getKeyStorePwd()).addKeyAlias(e.keyAlias).addKeyPwd(this.getKeyPwd()).addSignAlg(e.signAlg).addProfileFile(e.profile).addAppCertFile(e.certpath).addInputFile(this._signModel.getInputFilePath());
  }
}
