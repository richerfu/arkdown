/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/inner-java-command-builder/signing-options-v2.js
 */
import * as file_util_js_1 from "../../utils/file-util";
import * as java_command_builder_js_1 from "../java-command-builder";
export class SigningOptionsV2 extends java_command_builder_js_1.JavaCommandBuilder {
  sign() {
    return this.commandList.push("sign-app"), this;
  }
  addMode(i) {
    return this.commandList.push("-mode"), this.commandList.push(i), this;
  }
  addProfileFile(i) {
    return file_util_js_1.FileUtil.checkPathLength(i), this.commandList.push("-profileFile"), this.commandList.push(i), this;
  }
  profileSigned() {
    return this.commandList.push("-profileSigned"), this.commandList.push("1"), this;
  }
  addInForm(i) {
    return this.commandList.push("-inForm"), this.commandList.push(i), this;
  }
  addSignAlg(i) {
    return this.commandList.push("-signAlg"), this.commandList.push(i), this;
  }
  addKeyAlias(i) {
    return this.commandList.push("-keyAlias"), this.commandList.push(i), this;
  }
  addKeyStoreFile(i) {
    return file_util_js_1.FileUtil.checkPathLength(i), this.commandList.push("-keystoreFile"), this.commandList.push(i), this;
  }
  addKeyStorePwd(i) {
    return this.commandList.push("-keystorePwd"), this.commandList.push(i), this;
  }
  addKeyPwd(i) {
    return this.commandList.push("-keyPwd"), this.commandList.push(i), this;
  }
  addAppCertFile(i) {
    return file_util_js_1.FileUtil.checkPathLength(i), this.commandList.push("-appCertFile"), this.commandList.push(i), this;
  }
  addInputFile(i) {
    return file_util_js_1.FileUtil.checkPathLength(i), this.commandList.push("-inFile"), this.commandList.push(i), this;
  }
  addOutputFile(i) {
    return file_util_js_1.FileUtil.checkPathLength(i), this.commandList.push("-outFile"), this.commandList.push(i), this;
  }
}
