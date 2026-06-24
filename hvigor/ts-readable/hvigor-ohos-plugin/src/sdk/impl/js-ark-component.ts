/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/js-ark-component.js
 */
import path_1 from "path";
import * as node_command_builder_js_1 from "../../builder/node-command-builder";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
export class JsArkComponent {
  constructor(e) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(JsArkComponent.name), this.ts2abcVersion = "", this.ts2abcToolPath = path_1.resolve(e, "bin", "ark", "ts2abc.js");
  }
  getArkVersion(e, s) {
    if ("" !== this.ts2abcVersion) return this.ts2abcVersion;
    try {
      const t = new node_command_builder_js_1.NodeCommandBuilder().addJsPath(this.ts2abcToolPath);
      return e ? (t.getCompatibleTsVersion(e), s && t.getCompatibleSdkVersionStage(s)) : t.getTsVersion(), this._log.debug(t.build()), this.ts2abcVersion = new process_utils_js_1.ProcessUtils().executeSync(t.build(), {
        stdio: "pipe"
      }).stdout.toString(), this.ts2abcVersion;
    } catch (e) {
      return this._log.debug("Execute command error.Cannot get ark version."), "";
    }
  }
}
