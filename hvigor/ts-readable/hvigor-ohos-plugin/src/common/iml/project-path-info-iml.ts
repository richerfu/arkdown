/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/iml/project-path-info-iml.js
 */
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as inject_util_js_1 from "../../utils/inject-util";
export class ProjectPathInfoIml {
  constructor(t, e) {
    this._projectModel = t, this._productName = e, this._projectPath = t.getProjectDir();
  }
  getProjectBuildPath() {
    return path_1.resolve(inject_util_js_1.InjectUtil.getBuildCacheParentDir(this._projectPath, this._projectModel.getProject().getName(), false), build_directory_const_js_1.BuildDirConst.BUILD_ROOT);
  }
  getProjectOutputPath() {
    return path_1.resolve(this.getProjectBuildPath(), build_directory_const_js_1.BuildDirConst.BUILD_OUTPUTS, this._productName);
  }
  getProjectDebugSymbolPath() {
    return path_1.resolve(this.getProjectOutputPath(), build_directory_const_js_1.BuildDirConst.OUTPUT_SYMBOL);
  }
}
