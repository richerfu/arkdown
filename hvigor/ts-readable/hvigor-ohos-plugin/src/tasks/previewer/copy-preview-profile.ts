/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/previewer/copy-preview-profile.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as preview_hook_compile_resource_js_1 from "../hook/preview-hook-compile-resource";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class CopyPreviewProfile extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, CommonTask.COPY_PREVIEWER_PROFILE), this.profilePath = undefined, this.targetProfilePath = undefined, this.profilePath = path_1.resolve(this.targetData.getModuleModel().getSourceRootByTargetName(this.targetData.getModuleModel().getName()), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES, build_directory_const_js_1.BuildDirConst.BASE, build_directory_const_js_1.BuildDirConst.PROFILE), this.targetProfilePath = path_1.resolve(this.targetData.getPathInfo().getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES, build_directory_const_js_1.BuildDirConst.BASE, build_directory_const_js_1.BuildDirConst.PROFILE);
  }
  initTaskDepends() {
    this.declareDepends(preview_hook_compile_resource_js_1.PreviewHookCompileResource.name);
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet();
    return this.profilePath && fs_extra_1.existsSync(this.profilePath) && e.addEntry(this.profilePath), e;
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet();
    return this.targetProfilePath && e.addEntry(this.targetProfilePath), e;
  }
  doTaskAction() {
    undefined !== this.profilePath && fs_extra_1.existsSync(this.profilePath) && this.targetProfilePath && (fs_extra_1.ensureDirSync(this.targetProfilePath), fs_extra_1.copySync(this.profilePath, this.targetProfilePath, {
      recursive: true
    }));
  }
}
