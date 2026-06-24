/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/validate-sign.js
 */
import * as fse from "fs-extra";
import * as keystore_utils_1 from "../utils/keystore-utils";
import * as task_names_1 from "./common/task-names";
import * as ohos_hap_task_1 from "./task/ohos-hap-task";
import * as legacy_pre_build_js_1 from "./legacy-tasks/legacy-pre-build";
var CommonTask = task_names_1.TaskNames.CommonTask;
export class ValidateSigning extends ohos_hap_task_1.OhosHapTask {
  constructor(e) {
    super(e, CommonTask.VALIDATE_SIGNING);
  }
  initTaskDepends() {
    this.declareDepends(legacy_pre_build_js_1.LegacyPreBuild.name);
  }
  doTaskAction() {
    if (ValidateSigning.executed) return;
    const e = keystore_utils_1.KeyStoreHelper.getDefaultDebugKeyStoreLocation();
    undefined === e || fse.existsSync(e) || keystore_utils_1.KeyStoreHelper.createDefaultDebugStore(), ValidateSigning.executed = true;
  }
}
ValidateSigning.executed = false;
