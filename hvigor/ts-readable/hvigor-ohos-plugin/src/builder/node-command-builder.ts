/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/node-command-builder.js
 */
import * as file_util_js_1 from "../utils/file-util";
import * as base_command_builder_js_1 from "./base-command-builder";
export class NodeCommandBuilder extends base_command_builder_js_1.BaseCommandBuilder {
  constructor(s) {
    super([]), this._supportNewEnvArgs = s || false, this._envArgs = [], this.commandList.push(process.execPath);
  }
  extendBuilder(s) {
    return this.addAllParams(s.commandList), this._supportNewEnvArgs = s.isEtsLoader(), this._envArgs = s.getEnvArgs(), this;
  }
  addWebpackPath(s) {
    return file_util_js_1.FileUtil.checkPathLength(s), this.commandList.push(s), this;
  }
  addJsPath(s) {
    return file_util_js_1.FileUtil.checkPathLength(s), this.commandList.push(s), this;
  }
  getTsVersion() {
    return this.commandList.push("--bc-version"), this;
  }
  getCompatibleTsVersion(s) {
    return this.commandList.push("--target-api-version", s.toString()), this;
  }
  getCompatibleSdkVersionStage(s) {
    return this.commandList.push("--target-api-sub-version", s), this;
  }
  addWebpackConfig(s) {
    return this.commandList.push("--config"), this.commandList.push(s), this;
  }
  addTitle(s) {
    return this.commandList.push(`--title=${s}`), this;
  }
  addObfuscateType(s) {
    return this.addEnvArgs("obfuscate", s), this;
  }
  addDeviceType(s) {
    const t = s.join(",");
    return this._supportNewEnvArgs ? this.addEnvArgs("deviceType", t) : (this.commandList.push("--deviceType"), this.commandList.push(t)), this;
  }
  addBuildMode(s) {
    const t = s ? "debug" : "release";
    return this._supportNewEnvArgs ? this.addEnvArgs("buildMode", t) : (this.commandList.push("--buildMode"), this.commandList.push(t)), this;
  }
  addCompilerType(s) {
    return this._supportNewEnvArgs ? this.addEnvArgs("compilerType", s) : (this.commandList.push("--compilerType"), this.commandList.push(s)), this;
  }
  addOpenSslLegacyOption(s) {
    return s && this.commandList.push("--openssl-legacy-provider"), this;
  }
  build() {
    return this.appendEnvArgs(), super.build();
  }
  isEtsLoader() {
    return this._supportNewEnvArgs;
  }
  getEnvArgs() {
    return this._envArgs;
  }
  addEnvArgs(s, t) {
    this._envArgs.push(`${s}=${t}`);
  }
  appendEnvArgs() {
    this._supportNewEnvArgs && 0 !== this._envArgs.length && (-1 === this.commandList.indexOf("--env") && this.commandList.push("--env"), this.commandList = this.commandList.concat(this._envArgs), this._envArgs = []);
  }
}
