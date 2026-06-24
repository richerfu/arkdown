/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-device-coverage.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as remote_hsp_utils_js_1 from "../utils/remote-hsp-utils";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
import * as device_coverage_queue_center_js_1 from "./device-coverage-queue-center";
export class GenerateDeviceCoverage extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.GENERATE_DEVICE_COVERAGE), this._log = ohos_logger_js_1.OhosLogger.getLogger(GenerateDeviceCoverage.name);
  }
  async doTaskAction() {
    var e, t, o;
    if (this.targetData.getTargetName() === common_const_js_1.CommonConst.OHOS_TEST) return;
    this.targetService.getModuleService().findTargetDataByName(common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) || this._log.printErrorExit("NO_TARGET_OHOSTEST", [this.moduleModel.getName(), common_const_js_1.CommonConst.ON_DEVICE_TEST], [[this.moduleModel.getName()]]);
    const s = path_1.resolve(path_1.resolve(this.moduleModel.getProjectDir(), build_directory_const_js_1.BuildDirConst.SRC), common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET);
    fs_extra_1.existsSync(s) || this._log.printErrorExit("OHOSTEST_PATH_NOT_EXIST", [s, common_const_js_1.CommonConst.ON_DEVICE_TEST], [[s]]);
    const r = (0, remote_hsp_utils_js_1.getSignRemoteHspMetadata)(this.service, this.targetService.getTargetData().getProduct().name);
    let a = [];
    r && (a = r.map(e => e.hspPath));
    const _ = this.service.getProjectModel().getProjectDir(),
      i = this.moduleModel.getModule().getNodeDir(),
      n = path_1.resolve(_, i, `src/ohosTest/${common_const_js_1.CommonConst.MODULE_JSON5}`);
    fs_extra_1.existsSync(n) || this._log.printErrorExit("OHOSTEST_MODULE_JSON_NOT_EXIST", [n], [[n]]);
    const u = hvigor_1.Json5Reader.getJson5Obj(n),
      l = {
        toolchainsPath: this.sdkInfo.getBasePath(),
        projectPath: this.projectModel.getProjectDir(),
        modulePath: this.pathInfo.getModulePath(),
        bundleName: null !== (e = this.targetData.getProduct().bundleName) && undefined !== e ? e : this.projectModel.getDefaultBundleName(),
        bundleType: null !== (t = this.targetData.getProduct().bundleType) && undefined !== t ? t : this.projectModel.getBundleType(),
        coverage: "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.OHOS_TEST_COVERAGE),
        logger: this._log,
        scope: null !== (o = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.OHOS_TEST_COVERAGE_SCOPE)) && undefined !== o ? o : "",
        testModuleName: u.module.name,
        targetHapPath: this.getOutputPackagePath(),
        testHapPath: path_1.resolve(this.pathInfo.getModuleBuildOhosTestOutputPath(), this.targetData.getModuleTargetOutputFileName("", true, build_directory_const_js_1.BuildArtifactExtension.DOT_HAP, "", "ohosTest")),
        dependenciesPaths: [...this.getDependenciesPaths(), ...a].reverse()
      };
    await device_coverage_queue_center_js_1.deviceCoverageQueueCenter.executeProxy(l);
  }
  getOutputPackagePath() {
    var e;
    if (this.service.getModuleModel().getModuleType() === module_type_enum_js_1.ModuleType.Shared) return path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), this.targetData.getModuleTargetOutputFileName(this.targetName, true, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP));
    if (this.service.getModuleModel().getModuleType() === module_type_enum_js_1.ModuleType.Entry || this.service.getModuleModel().getModuleType() === module_type_enum_js_1.ModuleType.Feature) return path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), this.targetData.getModuleTargetOutputFileName(this.targetName, true, build_directory_const_js_1.BuildArtifactExtension.DOT_HAP));
    {
      const t = null === (e = this.targetData.getTargetOpt().output) || undefined === e ? undefined : e.artifactName,
        o = t ? `${t}` : `${this.moduleName}`;
      return path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), `${o}.har`);
    }
  }
  initTaskDepends() {}
  getDependenciesPaths() {
    const e = new Set();
    return new dependency_manager_js_1.DependencyManager(false, this.moduleModel, this.projectModel).collectAllHspDependencies().forEach(t => {
      const o = t[0];
      for (const t of [this.targetService.getTargetData().getTargetName(), common_const_js_1.DefaultTargetConst.DEFAULT_TARGET]) {
        const s = this.targetService.getTargetData().findTargetDataByName(o, t);
        undefined !== s && e.add(path_1.resolve(s.getPathInfo().getModuleBuildOutputPath(), s.getModuleTargetOutputFileName("", true, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP)));
      }
    }), Array.from(e);
  }
}
