/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/har-ark-compile.js
 */
import path_1 from "path";
import * as fse from "fs-extra";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as json_util_js_1 from "../../utils/json-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../utils/task-util";
import * as ark_compile_js_1 from "../ark-compile";
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as generate_loader_json_js_1 from "../generate-loader-json";
import * as core_process_package_json_js_1 from "./core-process-package-json";
import * as har_compile_node_js_1 from "./har-compile-node";
import * as ohos_uitransform_optimization_js_1 from "../module-info/ohos-uitransform-optimization";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class HarArkCompile extends ark_compile_js_1.ArkCompile {
  constructor(e, t, s) {
    super(e, t, s), this._log = ohos_logger_js_1.OhosLogger.getLogger(HarArkCompile.name), this.targetService.isByteCodeHar() || (this.aceModuleBuild = path_1.resolve(this.pathInfo.getInterMediatesLoaderOutPath()));
  }
  allowToPreloadSystemSo() {
    return this.isPreloadSystemSoEnabled && this.targetService.isByteCodeHar();
  }
  addPreloadSystemSoFileList() {
    this.addSelfPreloadSystemSoFileList();
  }
  declareInputs() {
    return super.declareInputs().set("isByteCodeHar", this.targetService.isByteCodeHar()).set("isBundledDependencies", this.targetService.isBundledDependencies());
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    if (core_process_package_json_js_1.packageJsonValidMainField) {
      const t = path_1.resolve(this.moduleModel.getProjectDir(), core_process_package_json_js_1.packageJsonValidMainField);
      fse.existsSync(t) && e.addEntry(t);
    }
    const t = this.pathInfo.getHarGenerateBuildProfilePath();
    return fse.existsSync(t) && e.addEntry(t), e;
  }
  async doTaskAction() {
    await super.doTaskAction();
  }
  async initDefaultArkCompileConfig() {
    var e, t, s;
    const i = this.targetService.isByteCodeHar();
    (null === (e = this.buildOption) || undefined === e ? undefined : e.debuggable) || i || !(null === (s = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.arkOptions) || undefined === s ? undefined : s.branchElimination) || this._log.warn(`Because you have set 'byteCodeHar' to false in ${this.moduleName}'s build-profile.json5, branchElimination will have no effect. Only in release mode and set 'byteCodeHar' to true, that the branchElimination will have effect.`);
    return {
      ...(await super.initDefaultArkCompileConfig()),
      byteCodeHar: i,
      obfuscate: this.targetService.isOpenSource() ? undefined : har_compile_node_js_1.ObfuscateType.UGLIFY,
      skipBuildEnd: !i && this.targetService.isOpenSource() && !(0, ohos_uitransform_optimization_js_1.getOhosUiTransformOptimization)(this.targetService),
      compileBlockPkg: this.getDependencyKeys()
    };
  }
  initTaskDepends() {
    (0, task_util_js_1.limitMinApiVersion)(this.targetData, sdk_const_js_1.ApiVersion.API_VERSION_9) ? this.declareDependsList(generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name, CommonTask.CREATE_HAR_BUILD_PROFILE.name) : this.declareDependsList(generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name);
  }
  getDependencyKeysByPkgPath(e) {
    var t, s, i;
    const r = json_util_js_1.JsonUtil.getJson5Obj(e);
    return r ? {
      ...(null !== (t = r.dependencies) && undefined !== t ? t : {}),
      ...(null !== (s = r.devDependencies) && undefined !== s ? s : {}),
      ...(null !== (i = r.dynamicDependencies) && undefined !== i ? i : {})
    } : {};
  }
  getDependencyKeys() {
    if (this.targetService.isBundledDependencies()) {
      return (this.isByteCodeHarOptimize ? this.service.getAffectsCompilationDependencies().filter(e => e.isHspDependency() || e.isOtherDependency() || e.isByteCodeHarDependency()) : this.service.getHspOrByteCodeHarDependencies().concat(this.service.getOtherDependencies())).map(e => e.getDependencyName());
    }
    const e = this.getDependencyKeysByPkgPath(this.moduleModel.getOhPackageJson5Path()),
      t = this.targetService.isByteCodeHar() ? {
        ...this.getDependencyKeysByPkgPath(this.projectModel.getOhPackageJson5Path()),
        ...e
      } : e;
    return Object.keys(t);
  }
}
