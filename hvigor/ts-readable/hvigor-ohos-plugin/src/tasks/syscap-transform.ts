/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/syscap-transform.js
 */
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as task_names_js_1 from "./common/task-names";
import * as res_model_loader_js_1 from "../utils/loader/file/res-model-loader";
import * as abstract_syscap_transform_js_1 from "./abstract/abstract-syscap-transform";
var Task = task_names_js_1.TaskNames.Task;
export class SyscapTransform extends abstract_syscap_transform_js_1.AbstractSyscapTransform {
  constructor(s) {
    super(s, Task.SYSCAP_TRANSFORM), this.jsonFilePath = path_1.resolve(this.sourceRoot, common_const_js_1.CommonConst.MODULE_JSON5);
  }
  getJsonProfileByModel() {
    const s = res_model_loader_js_1.resModelLoader.getModuleJson(this.jsonFilePath);
    return {
      jsonFilePath: this.jsonFilePath,
      profile: s,
      deviceTypes: s.module.deviceTypes,
      deviceConfig: common_const_js_1.CommonConst.DEVICE_TYPES,
      configurationProfile: common_const_js_1.CommonConst.MODULE_JSON5
    };
  }
}
