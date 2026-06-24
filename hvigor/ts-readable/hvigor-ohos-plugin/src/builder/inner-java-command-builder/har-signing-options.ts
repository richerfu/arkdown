/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/inner-java-command-builder/har-signing-options.js
 */
import * as file_util_js_1 from "../../utils/file-util";
import * as signing_options_v2_1 from "./signing-options-v2";
export class HarSigningOptions extends signing_options_v2_1.SigningOptionsV2 {
  sign() {
    return this.commandList.push("sign-har"), this;
  }
  addAppCertFile(i) {
    return file_util_js_1.FileUtil.checkPathLength(i), this.commandList.push("-signCertFile"), this.commandList.push(i), this;
  }
}
