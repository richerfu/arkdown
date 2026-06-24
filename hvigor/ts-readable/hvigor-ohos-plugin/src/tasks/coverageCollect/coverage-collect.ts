/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/coverageCollect/coverage-collect.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as coverage_collect_const_js_1 from "../../const/coverage-collect-const";
import * as task_names_js_1 from "../common/task-names";
var task = task_names_js_1.TaskNames.CommonTask;
const LOG = hvigor_1.HvigorLogger.getLogger("coverageCollect"),
  PROJECT_PATH = coverage_collect_const_js_1.CoverageCollectConst.PROJECT_PATH,
  REPORT_PATH = coverage_collect_const_js_1.CoverageCollectConst.REPORT_PATH,
  COVERAGE_FILE = coverage_collect_const_js_1.CoverageCollectConst.COVERAGE_FILE;
export class CoverageCollect extends hvigor_1.DefaultTask {
  constructor(e, o) {
    super(e, task.COLLECT_COVERAGE), this._taskService = o, this.projectPath = hvigor_1.hvigorCore.getExtraConfig().get(PROJECT_PATH), this.reportPath = hvigor_1.hvigorCore.getExtraConfig().get(REPORT_PATH), this.coverageFile = hvigor_1.hvigorCore.getExtraConfig().get(COVERAGE_FILE);
  }
  registryAction() {
    return async () => {
      LOG.info("Generate coverage statistics reports based on instrumentation dotting data"), await (0, hvigor_arkts_compose_1.generateReport)(this.projectPath, this.reportPath, this.coverageFile);
    };
  }
}
