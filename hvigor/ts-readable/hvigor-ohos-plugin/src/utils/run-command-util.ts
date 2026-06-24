/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/run-command-util.js
 */
import * as child_process_1 from "child_process";
import * as ohos_logger_1 from "./log/ohos-logger";
const isWindows = "win32" === process.platform,
  scriptShell = isWindows ? process.env.ComSpec || "cmd" : process.env.SHELL || "sh";
export class RunCommandUtil {
  static run(s, o = process.cwd(), e) {
    var r;
    if (!s) return this._log.error("", `Missing script: ${s}`), false;
    const i = isWindows ? ["/d", "/c", s] : ["-c", s],
      n = (0, child_process_1.spawnSync)(scriptShell, i, {
        windowsVerbatimArguments: true,
        stdio: "inherit",
        cwd: o,
        env: process.env
      });
    if (n && 0 === n.status) return true;
    if (e) {
      const s = null === (r = n.error) || undefined === r ? undefined : r.message;
      this._log.warn(`excute command failed, error is${s}`);
    }
    return false;
  }
}
RunCommandUtil._log = ohos_logger_1.OhosLogger.getLogger("RunCommandUtil");
