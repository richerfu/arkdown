/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-har-loader-json.js
 */
import * as fse from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as compile_mode_enum_js_1 from "../../enum/compile-mode-enum";
import * as abstract_generate_loader_json_js_1 from "../abstract-generate-loader-json";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_pre_build_js_1 from "./legacy-pre-build";
export class LegacyGenerateHarLoaderJson extends abstract_generate_loader_json_js_1.AbstractGenerateLoaderJson {
  declareOutputFiles() {
    return super.declareOutputFiles().addEntry(this.aceLoaderJson);
  }
  initTaskDepends() {
    this.declareDepends(legacy_pre_build_js_1.LegacyPreBuild.name);
  }
  constructor(e) {
    super(e, task_names_js_1.TaskNames.LegacyFATask.GENERATE_HAR_LOADER_JSON), this.aceLoaderJson = path_1.resolve(e.getTargetData().getPathInfo().getIntermediatesLoaderPath(), build_directory_const_js_1.BuildArtifactConst.LOADER_JSON);
  }
  doTaskAction() {
    const e = {
      modulePathMap: this.modulePathMap,
      compileMode: compile_mode_enum_js_1.CompileModeEnum.JS_BUNDLE,
      projectRootPath: this.projectRootPath,
      packageManagerType: this.isOhpmProject ? "ohpm" : "npm"
    };
    fse.outputJSONSync(this.aceLoaderJson, e);
  }
}
