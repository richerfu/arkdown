/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/module-task-service-factory.js
 */
import * as dependency_manager_js_1 from "../../project/dependency/dependency-manager";
import * as module_task_service_js_1 from "./module-task-service";
export class ModuleTaskServiceFactory {
  static initModuleTaskService(e, r, s) {
    const c = new dependency_manager_js_1.DependencyManager(s, r, e);
    return new module_task_service_js_1.ModuleTaskService(e, r, c, s);
  }
}
