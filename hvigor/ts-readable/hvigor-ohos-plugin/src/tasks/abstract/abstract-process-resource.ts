/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-process-resource.js
 */
import os_1 from "os";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as minimatch_1 from "minimatch";
import * as common_util_js_1 from "../../common/common-util";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_test_util_js_1 from "../../utils/ohos-test-util";
import * as legacy_process_profile_js_1 from "../legacy-tasks/legacy-process-profile";
import * as process_profile_js_1 from "../process-profile";
import * as abstract_resource_task_js_1 from "./abstract-resource-task";
const _log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-abstract-process-resource");
export class AbstractProcessResource extends abstract_resource_task_js_1.AbstractResource {
  constructor(e, s) {
    super(e, s), this.processedJson = this.isFaMode ? this.pathInfo.getIntermediatesProcessLegacyProfile() : this.pathInfo.getIntermediatesProcessProfile();
  }
  getEnabled() {
    return this.initCommandBuilder(), super.getEnabled();
  }
  initTaskDepends() {
    this.declareDepends(this.isFaMode ? legacy_process_profile_js_1.LegacyProcessProfile.name : process_profile_js_1.ProcessProfile.name);
  }
  doTaskAction() {
    const e = this.restoolConfigBuilder.getConfigObject();
    fs_extra_1.outputJSONSync(this.resConfigFilePath, e);
  }
  declareInputs() {
    return new Map().set("resConfigJsonContent", JSON.stringify(this.restoolConfigBuilder.getConfigObject()));
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntry(this.resConfigFilePath, {
      isDirectory: false
    });
  }
  isEnabledIconCheck() {
    const e = this.moduleModel.getModuleType();
    if (e !== module_type_enum_js_1.ModuleType.Entry && e !== module_type_enum_js_1.ModuleType.Feature) return false;
    let s = (0, common_util_js_1.isSingleFramework)() && this.targetData.getCompileApiVersion() >= sdk_const_js_1.ApiVersion.API_VERSION_10;
    const t = [device_type_const_js_1.DeviceTypeConst.DEFAULT, device_type_const_js_1.DeviceTypeConst.PHONE, device_type_const_js_1.DeviceTypeConst.TABLET],
      o = this.moduleModel.getJsonProfileByModel(this.service, this.targetName);
    s = s && !t.every(e => {
      var s;
      return !(null === (s = o.deviceTypes) || undefined === s ? undefined : s.includes(e));
    });
    const r = this.moduleModel.isInstallFree();
    if (this.isFaMode) return s && !r;
    const i = this.projectModel.getAppRes();
    return s && (i.getAppResOpt().app.bundleType === common_const_js_1.BundleType.APP || undefined === i.getAppResOpt().app.bundleType) && !r;
  }
  processResCompileThreads() {
    var e, s;
    const t = null === (s = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.resOptions) || undefined === s ? undefined : s.resCompileThreads;
    if (!t) return;
    const o = os_1.cpus().length;
    return t > o ? (_log.warn("resCompileThreads in build-profile.json5 exceeds the threads limit and will adapt automatically. "), o) : t;
  }
  processIgnoreResourcePattern(e) {
    if (e && Array.isArray(e)) return e.reduce((e, s) => {
      const t = minimatch_1.minimatch.makeRe(s);
      return t && e.push(t.source), e;
    }, []);
  }
  addOhosTestExtraCompilationResources(e) {
    const s = path_1.resolve(this.moduleModel.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES);
    fs_extra_1.existsSync(s) && e.addInputDir(s, "Module");
  }
  addIntermediatesSrcResource(e, s) {
    if ((0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(s)) {
      const s = this.pathInfo.getOhosTestIntermediatesSrcResource();
      fs_extra_1.existsSync(s) && e.addInputDir(s, "Module");
    }
  }
}
