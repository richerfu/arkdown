/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/debugger/build-hot-reload-resource.js
 */
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as compile_mode_enum_js_1 from "../../enum/compile-mode-enum";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as error_url_js_1 from "../../utils/error-url";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../utils/task-util";
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as generate_loader_json_js_1 from "../generate-loader-json";
import * as abstract_module_hook_task_js_1 from "../hook/abstract-module-hook-task";
export class BuildHotReloadResource extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(e) {
    super(e, false, task_names_js_1.TaskNames.CommonHookTask.BUILD_HOT_RELOAD_RES), this._log = ohos_logger_js_1.OhosLogger.getLogger(BuildHotReloadResource.name), this.registryAction = () => {
      const e = this._moduleService.getModuleModel(),
        o = (0, find_target_product_js_1.findTargetProduct)(e.getParentProject()),
        r = e.getCompileApiMetaByProduct(o.name).version;
      return () => {
        (e.getCompileMode(o) !== compile_mode_enum_js_1.CompileModeEnum.ES_MODULE || e.getApiType() !== hap_extra_info_js_1.ApiType.STAGE || r < 9) && this._log.printErrorExit("HOT_RELOAD_PROHIBIT", [e.getProfilePath()], [[error_url_js_1.ErrorUrl.ERROR_URL.hotReload]]), this._log.debug("Hot reload resources ready.");
      };
    };
  }
  initTaskDepends(e) {
    this.dependsOn((0, task_util_js_1.resolveHookDependency)(generate_loader_json_js_1.GenerateLoaderJson.name, e.getTargetData().getTargetName())), this.dependsOn((0, task_util_js_1.resolveHookDependency)(compile_resource_js_1.CompileResource.name, e.getTargetData().getTargetName()));
  }
}
