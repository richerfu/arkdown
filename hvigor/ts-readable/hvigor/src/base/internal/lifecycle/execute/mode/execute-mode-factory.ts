/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/execute/mode/execute-mode-factory.js
 */
import * as common_const_js_1 from "../../../../common/options/common-const";
import * as hvigor_log_js_1 from "../../../../log/hvigor-log";
import * as execute_module_pipeline_js_1 from "./execute-module-pipeline";
import * as execute_project_pipeline_js_1 from "./execute-project-pipeline";
import * as execute_undefined_pipeline_js_1 from "./execute-undefined-pipeline";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-execute-mode-factory");
export class ExecuteModeFactory {
  constructor(e, o) {
    this.project = e, this.taskOptions = o;
  }
  getExecutePipeline(e) {
    switch (e) {
      case common_const_js_1.HvigorBuildConst.PROJECT_MODE:
        return new execute_project_pipeline_js_1.ExecuteProjectPipeline(this.project, this.taskOptions, true);
      case common_const_js_1.HvigorBuildConst.MODULE_MODE:
        return new execute_module_pipeline_js_1.ExecuteModulePipeline(this.project, this.taskOptions);
      case undefined:
        return new execute_undefined_pipeline_js_1.ExecuteUndefinedPipeline(this.project, this.taskOptions);
      default:
        throw _log.error(`Unknown mode '${e}' is specified in the command line. Please check.`), new Error("Command status error.");
    }
  }
}
