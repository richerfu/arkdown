/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/before-generate-device-coverage.js
 */
import * as task_names_js_1 from "./common/task-names";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
export class BeforeGenerateDeviceCoverage extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.BEFORE_GENERATE_DEVICE_COVERAGE);
  }
  doTaskAction() {}
  initTaskDepends() {}
}
