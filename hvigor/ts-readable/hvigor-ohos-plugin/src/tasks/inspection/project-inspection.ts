/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/inspection/project-inspection.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class ProjectInspection {
  constructor(o) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger("Inspection"), this.projectModel = o;
  }
  doInspection() {
    return this.projectPathInspection(), this;
  }
  projectPathInspection() {
    const o = process.cwd();
    ((0, hvigor_1.isWindows)() ? /(([A-z]):)(\\[a-zA-Z0-9-_.()\s@]+)*(\\?)$/ : /^(\/([a-zA-Z0-9-_.()\s@]+))+(\/?)$/).test(o) || this._log.printErrorExit("INVALID_PROJECT_PATH", [o]);
  }
  exitOnError() {
    this._log._printAllExit();
  }
}
