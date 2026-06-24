/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-loader-json.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as ability_type_enum_js_1 from "../../enum/ability-type-enum";
import * as compile_mode_enum_js_1 from "../../enum/compile-mode-enum";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as abstract_generate_loader_json_js_1 from "../abstract-generate-loader-json";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_generate_js_manifest_js_1 from "./legacy-generate-js-manifest";
import * as legacy_pre_build_js_1 from "./legacy-pre-build";
export class LegacyGenerateLoaderJson extends abstract_generate_loader_json_js_1.AbstractGenerateLoaderJson {
  declareInputs() {
    return super.declareInputs().set("multiResourceBuildArray", JSON.stringify(this.multiResourceBuildArray));
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntries([...this.outputAceLoaderJsonArray]);
  }
  initTaskDepends() {
    this.declareDepends(legacy_pre_build_js_1.LegacyPreBuild.name), this.declareDepends(legacy_generate_js_manifest_js_1.LegacyGenerateJsManifest.name);
  }
  constructor(e) {
    super(e, task_names_js_1.TaskNames.LegacyFATask.GENERATE_LOADER_JSON), this.outputAceLoaderJsonArray = [], this.multiResourceBuildArray = [], this.isWorkerArray = [];
    const t = this.service.getModuleModel().getLegacyAbilities(this.targetName);
    for (let r = 0; r < t.length; r++) {
      const s = t[r],
        i = s.getType();
      if (i === ability_type_enum_js_1.AbilityTypeEnum.PAGE || i === ability_type_enum_js_1.AbilityTypeEnum.FORM || i === ability_type_enum_js_1.AbilityTypeEnum.WORKER || i === ability_type_enum_js_1.AbilityTypeEnum.DATA || i === ability_type_enum_js_1.AbilityTypeEnum.SERVICE) {
        const t = path_1.resolve(e.getTargetData().getPathInfo().getIntermediatesLoaderPath(), s.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LOADER_JSON);
        this.outputAceLoaderJsonArray.push(t), this.isWorkerArray.push(i === ability_type_enum_js_1.AbilityTypeEnum.WORKER);
      }
    }
  }
  shouldLoaderWorker(e) {
    return !!inject_util_js_1.InjectUtil.isPreviewProcess() || (this.compileApiVersion > sdk_const_js_1.ApiVersion.API_VERSION_8 && this.workerConfig && this.workerConfig.filter(e => ".ts" === path_1.extname(e)).length > 0 ? this.isWorkerArray[e] : 0 === e);
  }
  doTaskAction() {
    const e = this.service.getModuleModel().getLegacyAbilities(this.targetName);
    for (let t = 0; t < e.length; t++) {
      const r = e[t];
      this.multiResourceBuildArray.push(this.getMultiResourceBuild(this.targetService.getTargetData(), r));
    }
    for (let e = 0; e < this.outputAceLoaderJsonArray.length; e++) {
      const t = {
        workers: this.shouldLoaderWorker(e) ? this.workerConfig : undefined,
        multiResourceBuild: this.multiResourceBuildArray[e],
        modulePathMap: this.modulePathMap,
        compileMode: compile_mode_enum_js_1.CompileModeEnum.JS_BUNDLE,
        projectRootPath: this.projectRootPath,
        packageManagerType: this.isOhpmProject ? "ohpm" : "npm"
      };
      fse.outputJSONSync(this.outputAceLoaderJsonArray[e], t);
    }
  }
  getMultiResourceBuild(e, t) {
    const r = e.findTargetConfigOpt(t);
    if (r) return r.pages ? {
      pages: r.pages,
      res: r.res
    } : {
      pages: this.getManifestPage(e, t),
      res: r.res
    };
  }
  getManifestPage(e, t) {
    const r = path_1.resolve(this.pathInfo.getIntermediatesLegacyManifestJson(), t.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LEGACY_MANIFEST_JSON);
    if (fse.existsSync(r)) return [];
    return fse.readJSONSync(r).pages;
  }
}
