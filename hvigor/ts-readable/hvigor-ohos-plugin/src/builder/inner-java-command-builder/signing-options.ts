/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/inner-java-command-builder/signing-options.js
 */
import * as file_util_js_1 from "../../utils/file-util";
import * as java_command_builder_js_1 from "../java-command-builder";
export class SigningOptions extends java_command_builder_js_1.JavaCommandBuilder {
  sign() {
    return this.commandList.push("sign"), this;
  }
  addMode(s) {
    return this.commandList.push("-mode"), this.commandList.push(s), this;
  }
  addProfile(s) {
    return file_util_js_1.FileUtil.checkPathLength(s), this.commandList.push("-profile"), this.commandList.push(s), this;
  }
  addSignAlg(s) {
    return this.commandList.push("-signAlg"), this.commandList.push(s), this;
  }
  addPrivateKey(s) {
    return this.commandList.push("-privatekey"), this.commandList.push(s), this;
  }
  profileSigned() {
    return this.commandList.push("-profileSigned"), this.commandList.push("1"), this;
  }
  addKeyStore(s) {
    return file_util_js_1.FileUtil.checkPathLength(s), this.commandList.push("-keystore"), this.commandList.push(s), this;
  }
  addKeyStorePwd(s) {
    return this.commandList.push("-keystorepasswd"), this.commandList.push(s), this;
  }
  addKeyAliasPwd(s) {
    return this.commandList.push("-keyaliaspasswd"), this.commandList.push(s), this;
  }
  addCertPath(s) {
    return file_util_js_1.FileUtil.checkPathLength(s), this.commandList.push("-certpath"), this.commandList.push(s), this;
  }
  addKs(s) {
    return this.commandList.push("--ks"), this.commandList.push(s), this;
  }
  addKsPass(s) {
    return this.commandList.push("--ks-pass"), this.commandList.push(s), this;
  }
  addKsKeyAlias(s) {
    return this.commandList.push("--ks-key-alias"), this.commandList.push(s), this;
  }
  addKeyPass(s) {
    return this.commandList.push("--key-pass"), this.commandList.push(s), this;
  }
  addOut(s) {
    return file_util_js_1.FileUtil.checkPathLength(s), this.commandList.push("--out"), this.commandList.push(s), this;
  }
  addInputFile(s) {
    return file_util_js_1.FileUtil.checkPathLength(s), this.commandList.push("-inputFile"), this.commandList.push(s), this;
  }
  addOutputFile(s) {
    return file_util_js_1.FileUtil.checkPathLength(s), this.commandList.push("-outputFile"), this.commandList.push(s), this;
  }
  add(s) {
    return this.commandList.push(s), this;
  }
}
