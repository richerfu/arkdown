/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/har/legacy-package-har.js
 */
import * as abstract_package_har_js_1 from "../../abstract/abstract-package-har";
import * as task_names_js_1 from "../../common/task-names";
import * as legacy_process_har_artifacts_js_1 from "./legacy-process-har-artifacts";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyPackageHar extends abstract_package_har_js_1.AbstractPackageHar {
  constructor(a) {
    super(a, LegacyFATask.PACKAGE_HAR), this.taskTmpDir = this.getTaskTempDir(this.targetData, undefined, false);
  }
  initTaskDepends() {
    this.declareDepends(legacy_process_har_artifacts_js_1.LegacyProcessHarArtifacts.name);
  }
}
