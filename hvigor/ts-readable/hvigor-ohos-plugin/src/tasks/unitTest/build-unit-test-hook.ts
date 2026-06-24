/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/build-unit-test-hook.js
 */
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as task_names_js_1 from "../common/task-names";
import * as generate_loader_json_js_1 from "../generate-loader-json";
import * as legacy_compile_resource_js_1 from "../legacy-tasks/legacy-compile-resource";
import * as legacy_generate_js_manifest_js_1 from "../legacy-tasks/legacy-generate-js-manifest";
import * as legacy_generate_loader_json_js_1 from "../legacy-tasks/legacy-generate-loader-json";
import * as legacy_process_resource_js_1 from "../legacy-tasks/legacy-process-resource";
import * as pre_check_syscap_js_1 from "../pre-check-syscap";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as replace_unit_test_index_file_js_1 from "./replace-unit-test-index-file";
export class BuildUnitTestHook extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e, s) {
    var t;
    super(e, task_names_js_1.TaskNames.Task.BUILD_UNIT_TEST_RES), this.isFaMode = s, this.syscapJsonPath = path_1.resolve(null === (t = this.service.getModuleModel()) || undefined === t ? undefined : t.getSourceRootByTargetName(), common_const_js_1.CommonConst.SYSCAP_JSON);
  }
  doTaskAction() {}
  initTaskDepends() {
    this.isFaMode ? (this.declareDepends(legacy_generate_loader_json_js_1.LegacyGenerateLoaderJson.name), this.declareDepends(legacy_generate_js_manifest_js_1.LegacyGenerateJsManifest.name), this.getLegacyUnitTestCompileRes()) : (this.declareDepends(generate_loader_json_js_1.GenerateLoaderJson.name), this.declareDepends(replace_unit_test_index_file_js_1.ReplaceUnitTestIndexFile.name)), this.declareDepends(pre_check_syscap_js_1.PreCheckSyscap.name);
  }
  getLegacyUnitTestCompileRes() {
    const e = this.service.getRelatedEntryModules(),
      s = this.service.getModuleModel().getModuleType();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(s, e)) for (const s of e) this.createTaskAndDepends(s);else this.createTaskAndDepends();
  }
  createTaskAndDepends(e) {
    const s = new legacy_process_resource_js_1.LegacyProcessResource(this.targetService, e),
      t = new legacy_compile_resource_js_1.LegacyCompileResource(this.targetService, e);
    this.module.registry(s), this.module.registry(t), s.initTaskDepends(), t.initTaskDepends(), this.declareDepends(t.getName());
  }
}
