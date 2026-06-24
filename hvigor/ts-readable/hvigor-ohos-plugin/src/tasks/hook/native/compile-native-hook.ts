/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/native/compile-native-hook.js
 */
import * as build_native_with_cmake_js_1 from "../../build-native-with-cmake";
import * as task_names_js_1 from "../../common/task-names";
import * as abstract_module_hook_task_js_1 from "../abstract-module-hook-task";
var CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
export class CompileNativeHook extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(e) {
    super(e, true, CommonHookTask.COMPILE_NATIVE);
  }
  initTaskDepends(e) {
    this.dependsOn(`${e.getTargetData().getTargetName()}@${build_native_with_cmake_js_1.BuildNativeWithCmake.name}`);
  }
}
