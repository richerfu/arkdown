/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/device-test-hook.js
 */
import * as find_target_product_js_1 from "../common/find-target-product";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as abstract_module_hook_task_js_1 from "./hook/abstract-module-hook-task";
export class DeviceTestHook extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  initTaskDepends(e) {}
  constructor(e, o) {
    super(e, o, task_names_js_1.TaskNames.Task.ON_DEVICE_TEST), this._log = ohos_logger_js_1.OhosLogger.getLogger("device-test-hook"), this.registryAction = () => () => {
      const e = this._moduleService.getModuleModel(),
        o = this._moduleService.getProjectModel(),
        t = (0, find_target_product_js_1.findTargetProduct)(o),
        s = this._moduleService.getTargets().map(e => e.getTargetName());
      e.isHarModule() || !s.includes(common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) || s.includes(common_const_js_1.DefaultTargetConst.DEFAULT_TARGET) || this._log.warn(`Ensure that the target of the ${e.getModule().getName()} module is default and is associated with the ${t.name} product through the applyToProducts configuration.\n        At file: ${o.getProfilePath()}`);
    };
  }
}
