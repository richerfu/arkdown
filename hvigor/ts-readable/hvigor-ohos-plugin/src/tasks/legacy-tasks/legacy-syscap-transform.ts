/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-syscap-transform.js
 */
import * as abstract_syscap_transform_js_1 from "../abstract/abstract-syscap-transform";
import * as task_names_js_1 from "../common/task-names";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacySyscapTransform extends abstract_syscap_transform_js_1.AbstractSyscapTransform {
  constructor(s) {
    super(s, LegacyFATask.SYSCAP_TRANSFORM), this.jsonFilePath = path_1.resolve(this.sourceRoot, common_const_js_1.CommonConst.CONFIG_JSON);
  }
  getJsonProfileByModel() {
    const s = res_model_loader_js_1.resModelLoader.getConfigJson(this.jsonFilePath);
    return {
      jsonFilePath: this.jsonFilePath,
      profile: s,
      deviceTypes: s.module.deviceType,
      deviceConfig: common_const_js_1.CommonConst.DEVICE_TYPE,
      configurationProfile: common_const_js_1.CommonConst.CONFIG_JSON
    };
  }
}
