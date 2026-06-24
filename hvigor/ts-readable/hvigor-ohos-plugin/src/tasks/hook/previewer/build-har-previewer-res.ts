/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/previewer/build-har-previewer-res.js
 */
import * as task_util_js_1 from "../../../utils/task-util";
import * as generate_loader_json_js_1 from "../../generate-loader-json";
import * as legacy_generate_har_js_manifest_js_1 from "../../legacy-tasks/legacy-generate-har-js-manifest";
import * as legacy_generate_har_loader_json_js_1 from "../../legacy-tasks/legacy-generate-har-loader-json";
import * as pre_check_syscap_js_1 from "../../pre-check-syscap";
import * as preview_hook_compile_resource_js_1 from "../preview-hook-compile-resource";
import * as abstract_previewer_res_js_1 from "./abstract-previewer-res";
export class BuildHarPreviewerRes extends abstract_previewer_res_js_1.AbstractPreviewerRes {
  constructor(e, s) {
    super(e, s);
  }
  initTaskDepends(e) {
    const s = e.getTargetData().getTargetName();
    this._isFaMode ? (this.dependsOn((0, task_util_js_1.resolveHookDependency)(legacy_generate_har_loader_json_js_1.LegacyGenerateHarLoaderJson.name, s)), this.dependsOn((0, task_util_js_1.resolveHookDependency)(legacy_generate_har_js_manifest_js_1.LegacyGenerateHarJsManifest.name, s))) : (this.dependsOn((0, task_util_js_1.resolveHookDependency)(generate_loader_json_js_1.GenerateLoaderJson.name, s)), this.dependsOn((0, task_util_js_1.resolveHookDependency)(preview_hook_compile_resource_js_1.PreviewHookCompileResource.name, s))), this.dependsOn((0, task_util_js_1.resolveHookDependency)(pre_check_syscap_js_1.PreCheckSyscap.name, s));
  }
}
