/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/npm-command-builder.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as file_util_js_1 from "../utils/file-util";
import * as base_command_builder_js_1 from "./base-command-builder";
export class NpmCommandBuilder extends base_command_builder_js_1.BaseCommandBuilder {
  constructor(t) {
    super([]);
    const i = (0, hvigor_1.isWindows)() ? "npm.cmd" : "npm",
      s = t ? path_1.join(t, i) : i;
    this.commandList.push(s);
  }
  runScript(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push("run"), this.commandList.push(t), this;
  }
  addScriptsOptions() {
    return this.commandList.push("--scripts-prepend-node-path"), this.commandList.push("true"), this;
  }
  install() {
    return this.commandList.push("install"), this;
  }
}
