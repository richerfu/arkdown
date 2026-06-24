/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/java-command-builder.js
 */
import * as file_util_js_1 from "../utils/file-util";
import * as base_command_builder_js_1 from "./base-command-builder";
export class JavaCommandBuilder extends base_command_builder_js_1.BaseCommandBuilder {
  constructor() {
    super([]), this.commandList.push("java"), this._javaSystemList = [];
  }
  addCalledJarFile(s) {
    if (file_util_js_1.FileUtil.checkPathLength(s), 1 === this.commandList.length) this.commandList.push("-jar"), this.commandList.push(s);else {
      const t = this.commandList.slice(0, 1),
        i = this.commandList.slice(1);
      this.commandList = [...t, "-jar", s, ...i];
    }
    return this;
  }
  addJvmOption(s) {
    return this.commandList.push(s), this;
  }
  build() {
    return [...this.commandList.slice(0, 1), ...this._javaSystemList, ...this.commandList.slice(1)];
  }
  setJavaSystemProperty(s, t) {
    return this._javaSystemList.push(`-D${s}=${t}`), this;
  }
}
