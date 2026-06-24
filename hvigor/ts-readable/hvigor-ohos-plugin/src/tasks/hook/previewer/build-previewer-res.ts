/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/previewer/build-previewer-res.js
 */
import * as task_util_js_1 from "../../../utils/task-util";
import * as generate_loader_json_js_1 from "../../generate-loader-json";
import * as legacy_hook_compile_resource_js_1 from "../../legacy-tasks/hook/legacy-hook-compile-resource";
import * as legacy_generate_js_manifest_js_1 from "../../legacy-tasks/legacy-generate-js-manifest";
import * as legacy_generate_loader_json_js_1 from "../../legacy-tasks/legacy-generate-loader-json";
import * as pre_check_syscap_js_1 from "../../pre-check-syscap";
import * as replace_previewer_page_js_1 from "../../previewer/replace-previewer-page";
import * as abstract_previewer_res_js_1 from "./abstract-previewer-res";
export class BuildPreviewerRes extends abstract_previewer_res_js_1.AbstractPreviewerRes {
  constructor(e, s) {
    super(e, s);
  }
  initTaskDepends(e) {
    this._isFaMode ? (this.dependsOn((0, task_util_js_1.resolveHookDependency)(legacy_generate_loader_json_js_1.LegacyGenerateLoaderJson.name, e.getTargetData().getTargetName())), this.dependsOn((0, task_util_js_1.resolveHookDependency)(legacy_generate_js_manifest_js_1.LegacyGenerateJsManifest.name, e.getTargetData().getTargetName())), this.dependsOn((0, task_util_js_1.resolveHookDependency)(legacy_hook_compile_resource_js_1.LegacyHookCompileResource.name, e.getTargetData().getTargetName()))) : (this.dependsOn((0, task_util_js_1.resolveHookDependency)(generate_loader_json_js_1.GenerateLoaderJson.name, e.getTargetData().getTargetName())), this.dependsOn((0, task_util_js_1.resolveHookDependency)(replace_previewer_page_js_1.ReplacePreviewerPage.name, e.getTargetData().getTargetName()))), this.dependsOn((0, task_util_js_1.resolveHookDependency)(pre_check_syscap_js_1.PreCheckSyscap.name, e.getTargetData().getTargetName()));
  }
}
