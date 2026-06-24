/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/ets-ark-component.js
 */
import path_1 from "path";
import * as node_command_builder_js_1 from "../../builder/node-command-builder";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
export class EtsArkComponent {
  constructor(e) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(EtsArkComponent.name), this.ets2abcVersion = "", this._ets2abcToolPath = path_1.resolve(e, "bin", "ark", "ts2abc.js");
  }
  getArkVersion(e, t) {
    if ("" !== this.ets2abcVersion) return this.ets2abcVersion;
    try {
      const s = new node_command_builder_js_1.NodeCommandBuilder().addJsPath(this._ets2abcToolPath);
      return e ? (s.getCompatibleTsVersion(e), t && s.getCompatibleSdkVersionStage(t)) : s.getTsVersion(), this._log.debug(s.build()), this.ets2abcVersion = new process_utils_js_1.ProcessUtils().executeSync(s.build(), {
        stdio: "pipe"
      }).stdout.toString(), this.ets2abcVersion.trim().split("[")[0];
    } catch (e) {
      return this._log.debug("Execute command error.Cannot get ark version."), "";
    }
  }
}
