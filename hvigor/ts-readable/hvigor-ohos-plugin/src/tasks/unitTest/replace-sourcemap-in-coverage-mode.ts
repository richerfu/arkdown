/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/replace-sourcemap-in-coverage-mode.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as task_names_js_1 from "../common/task-names";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as ohos_module_task_js_1 from "../task/ohos-module-task";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as sourcemap_util_js_1 from "../../utils/sourcemap-util";
var __decorate = this && this.__decorate || function (e, t, o, a) {
  var r,
    s = arguments.length,
    i = s < 3 ? t : null === a ? a = Object.getOwnPropertyDescriptor(t, o) : a;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) i = Reflect.decorate(e, t, o, a);else for (var u = e.length - 1; u >= 0; u--) (r = e[u]) && (i = (s < 3 ? r(i) : s > 3 ? r(t, o, i) : r(t, o)) || i);
  return s > 3 && i && Object.defineProperty(t, o, i), i;
};
export class ReplaceSourcemapInCoverageMode extends ohos_module_task_js_1.OhosModuleTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.REPLACE_SOURCEMAP_IN_COVERAGE_MODE), this.unitTestSourceMapPath = path_1.join(this.pathInfo.getModulePath(), ".test/default/intermediates/test/coverage_sourcemap/unitTestSourceMap.map"), this.defaultSourceMapPath = path_1.join(this.pathInfo.getInterMediatesAssetsPath(), "ets", build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP);
  }
  async doTaskAction() {
    inject_util_js_1.InjectUtil.isOhosTestCoverage() && (await (0, sourcemap_util_js_1.overWriteSourceMap)(this.defaultSourceMapPath, this.unitTestSourceMapPath));
  }
  initTaskDepends() {}
}
__decorate([(0, hvigor_1.InputFile)()], ReplaceSourcemapInCoverageMode.prototype, "unitTestSourceMapPath", undefined), __decorate([(0, hvigor_1.OutputFile)()], ReplaceSourcemapInCoverageMode.prototype, "defaultSourceMapPath", undefined);
