/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/arkts-task-initializer.js
 */
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as har_target_util_js_1 from "../../utils/har-target-util";
import * as task_util_js_1 from "../../utils/task-util";
import * as ark_compile_js_1 from "../ark-compile";
import * as cold_reload_ark_compile_js_1 from "../cold-reload-ark-compile";
import * as task_names_js_1 from "../common/task-names";
import * as compile_node_js_1 from "../compile-node";
import * as compile_resource_js_1 from "../compile-resource";
import * as generate_loader_json_js_1 from "../generate-loader-json";
import * as collect_duplicate_har_entries_ark_compile_js_1 from "../har-deduplicate/collect-duplicate-har-entries-ark-compile";
import * as har_ark_compile_js_1 from "../har/har-ark-compile";
import * as har_compile_node_js_1 from "../har/har-compile-node";
import * as hot_reload_ark_compile_js_1 from "../hot-reload-ark-compile";
import * as ohos_test_ark_compile_js_1 from "../ohosTest/ohos-test-ark-compile";
import * as ohos_test_compile_node_js_1 from "../ohosTest/ohos-test-compile-node";
import * as previewer_ark_compile_js_1 from "../previewer-ark-compile";
import * as task_creator_js_1 from "../task-creator";
import * as unit_test_ark_compile_js_1 from "../unitTest/unit-test-ark-compile";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export const initializeArkTsTasks = (e, s) => {
  e.getModuleService().getModuleModel().getModuleType() === module_type_enum_js_1.ModuleType.Har ? (s.registry(new ArkTsHarTaskCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new NodeHarBuildTaskCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new ArkTsTaskCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new NodeBuildTaskCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new NodeBuildTaskCA(e, code_type_enum_js_1.CodeType.JS))) : (s.registry(new ArkTsTaskCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new ArkTsTaskCA(e, code_type_enum_js_1.CodeType.JS)), s.registry(new NodeBuildTaskCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new NodeBuildTaskCA(e, code_type_enum_js_1.CodeType.JS))), s.registry(new ArkTsHotReloadCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new ArkTsColdReloadCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new ArkTsPreviewCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new ArkTsOhosTestCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new ArkTsOhosTestCA(e, code_type_enum_js_1.CodeType.JS)), s.registry(new NodeOhosTestBuildTaskCA(e, code_type_enum_js_1.CodeType.ETS)), s.registry(new NodeOhosTestBuildTaskCA(e, code_type_enum_js_1.CodeType.JS)), s.registry(new ArkTsUnitTestCA(e, code_type_enum_js_1.CodeType.ETS));
};
export class ArkTsTaskCA extends task_creator_js_1.TargetTaskCreator {
  constructor(e, s) {
    super(e), this.provider = () => new ark_compile_js_1.ArkCompile(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.COMPILE_ARK), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.COMPILE_ARK), this.declareDepends = () => this.targetService.shouldEnableHarDeduplicate() && this.targetService.getModuleService().getModuleModel().isHspModule() ? [`${task_names_js_1.TaskNames.Task.COLLECT_DEDUPLICATE_HAR_ENTRIES.name}ArkTS`] : [generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name].concat(getBuildProfileDependence(this.targetService)), this.codeType = s;
  }
}
export class ArkTsHarTaskCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new har_ark_compile_js_1.HarArkCompile(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.HAR_COMPILE), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.HAR_COMPILE), this.declareDepends = () => (0, task_util_js_1.limitMinApiVersion)(this.targetService.getTargetData(), sdk_const_js_1.ApiVersion.API_VERSION_9) ? [generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name, CommonTask.CREATE_HAR_BUILD_PROFILE.name] : [generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name];
  }
}
export class ArkTsOhosTestCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new ohos_test_ark_compile_js_1.OhosTestArkCompile(this.targetService, this.codeType), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.OHOS_TEST_ARK_COMPILE);
  }
}
export class ArkTsUnitTestCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new unit_test_ark_compile_js_1.UnitTestArkCompile(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.UNIT_TEST), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.UNIT_TEST), this.declareDepends = () => [task_names_js_1.TaskNames.Task.BUILD_UNIT_TEST_RES.name, task_names_js_1.TaskNames.CommonTask.UNIT_TEST_PROCESS_PROFILE.name].concat(getBuildProfileDependence(this.targetService));
  }
}
export class ArkTsPreviewCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new previewer_ark_compile_js_1.PreviewerArkCompile(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.PREVIEW), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.PREVIEW), this.declareDepends = () => {
      const e = [task_names_js_1.TaskNames.Task.PREVIEW_UPDATE_ASSETS.name];
      return (0, task_util_js_1.limitMinApiVersion)(this.targetService.getTargetData(), sdk_const_js_1.ApiVersion.API_VERSION_9) && e.push(task_names_js_1.TaskNames.CommonTask.CREATE_BUILD_PROFILE.name), e;
    };
  }
}
export class ArkTsHotReloadCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new hot_reload_ark_compile_js_1.HotReloadArkCompile(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.HOT_RELOAD), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.HOT_RELOAD), this.declareDepends = () => (0, task_util_js_1.declareReloadDepends)(this.targetService), this.afterConfigure = () => {
      this.depends = this.declareDepends().map(e => e.includes(":") ? e : `${this.node.getName()}:${e}`);
    };
  }
}
export class ArkTsHarDeduplicateCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new collect_duplicate_har_entries_ark_compile_js_1.CollectDuplicateHarEntriesArkCompile(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.COLLECT_DEDUPLICATE_HAR_ENTRIES), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.COLLECT_DEDUPLICATE_HAR_ENTRIES), this.declareDepends = () => [generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name].concat(getBuildProfileDependence(this.targetService));
  }
}
export class ArkTsColdReloadCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new cold_reload_ark_compile_js_1.ColdReloadArkCompile(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.COLD_RELOAD), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.COLD_RELOAD), this.declareDepends = () => [];
  }
}
export class NodeBuildTaskCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new compile_node_js_1.CompileNode(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.COMPILE_NODE), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.COMPILE_NODE), this.declareDepends = () => [generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name];
  }
}
export class NodeHarBuildTaskCA extends NodeBuildTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new har_compile_node_js_1.HarCompileNode(this.targetService, this.codeType, task_names_js_1.TaskNames.Task.HAR_BUILD), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.HAR_BUILD);
  }
}
export class NodeOhosTestBuildTaskCA extends ArkTsTaskCA {
  constructor() {
    super(...arguments), this.provider = () => new ohos_test_compile_node_js_1.OhosTestCompileNode(this.targetService, this.codeType), this.declareTaskDetail = () => getLangCompileTaskName(this.codeType, task_names_js_1.TaskNames.Task.OHOS_TEST_COMPILE_NODE);
  }
}
const getLangCompileTaskName = (e, s) => ({
    ...s,
    name: `${s.name}${e === code_type_enum_js_1.CodeType.ETS ? "ArkTS" : e.toUpperCase()}`
  }),
  getBuildProfileDependence = e => {
    const s = [];
    if ((0, task_util_js_1.limitMinApiVersion)(e.getTargetData(), sdk_const_js_1.ApiVersion.API_VERSION_9)) {
      s.push(CommonTask.CREATE_BUILD_PROFILE.name);
      har_target_util_js_1.HarTargetUtil.calDepHarTargets(e).forEach((e, a) => s.push(`${a}:${e}@${CommonTask.CREATE_HAR_BUILD_PROFILE.name}`));
    }
    return s;
  };
