/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/hapv2-sign-command-builder.js
 */
import * as common_sign_command_builder_1 from "./common-sign-command-builder";
export class Hapv2SignCommandBuilder extends common_sign_command_builder_1.CommonSignCommandBuilder {
  constructor(e, i, d, n, o) {
    super(e, i, d, n, o);
  }
  getSignTool() {
    return this._sdkInfo.getHapSignToolV2();
  }
  initCommandParams(e) {
    this.validateSignType("HarmonyOS"), this._signingOptions.sign().addMode(Hapv2SignCommandBuilder.SIGN_MODE).addKeyStore(e.storeFile).addKeyStorePwd(this.getKeyStorePwd()).addPrivateKey(e.keyAlias).addKeyAliasPwd(this.getKeyPwd()).addSignAlg(e.signAlg).addProfile(e.profile).addCertPath(e.certpath).addInputFile(this._signModel.getInputFilePath()).addOutputFile(this._signModel.getOutPutFilePath());
  }
}
Hapv2SignCommandBuilder.SIGN_MODE = "localjks";
