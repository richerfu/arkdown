/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/resource-task-initializer.js
 */
import * as common_const_js_1 from "../../const/common-const";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as ohos_test_util_js_1 from "../../utils/ohos-test-util";
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as process_id_defined_file_js_1 from "../har-deduplicate/process-id-defined-file";
import * as legacy_process_profile_js_1 from "../legacy-tasks/legacy-process-profile";
import * as generate_ohos_test_template_js_1 from "../ohosTest/generate-ohos-test-template";
import * as process_profile_js_1 from "../process-profile";
import * as process_resource_increment_js_1 from "../process-resource-increment";
import * as process_resource_js_1 from "../process-resource";
import * as process_router_map_js_1 from "../process-router-map";
import * as process_share_config_js_1 from "../process-share-config";
import * as process_startup_config_js_1 from "../process-startup-config";
import * as task_creator_js_1 from "../task-creator";
import * as unit_test_process_profile_js_1 from "../unitTest/unit-test-process-profile";
import * as task_creation_manager_js_1 from "./task-creation-manager";
export const initResourceTasks = (e, s) => {
  e.registry(new ProcessResourceCA(s)), e.registry(new CompileResourceCA(s)), e.registry(new ProcessResourceIncrementCA(s)), s.shouldEnableHarDeduplicate() && e.registry(new task_creation_manager_js_1.ProcessIdDefinedFileCA(s));
};
export class ProcessResourceCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_RESOURCE, this.provider = () => new process_resource_js_1.ProcessResource(this.targetService);
  }
  declareDepends() {
    const e = [this.isFa ? legacy_process_profile_js_1.LegacyProcessProfile.name : process_profile_js_1.ProcessProfile.name];
    return inject_util_js_1.InjectUtil.isUnitTestProcess() && e.push(unit_test_process_profile_js_1.UnitTestProcessProfile.name), this.targetService.shouldEnableHarDeduplicate() && e.push(process_id_defined_file_js_1.ProcessIdDefinedFile.name), e;
  }
}
export class CompileResourceCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.COMPILE_RESOURCE, this.provider = () => new compile_resource_js_1.CompileResource(this.targetService);
  }
  declareDepends() {
    const e = [process_resource_js_1.ProcessResource.name, process_router_map_js_1.ProcessRouterMap.name, process_share_config_js_1.ProcessShareConfig.name, process_startup_config_js_1.ProcessStartupConfig.name],
      s = this.targetService.getTargetData().getTargetName();
    if (s !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) return e;
    const r = this.targetService.getModuleService().getModuleModel().getSourceRootByTargetName(s);
    return (0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(r) && e.push(generate_ohos_test_template_js_1.GenerateOhosTestTemplate.name), e;
  }
}
export class ProcessResourceIncrementCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [process_profile_js_1.ProcessProfile.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PREVIEW_PROCESS_RESOURCE, this.provider = () => new process_resource_increment_js_1.ProcessResourceIncrement(this.targetService);
  }
}
