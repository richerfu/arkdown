/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/har/legacy-process-har-artifacts.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as abstract_process_har_artifacts_js_1 from "../../abstract/abstract-process-har-artifacts";
import * as cache_native_libs_js_1 from "../../cache-native-libs";
import * as legacy_hook_compile_resource_js_1 from "../hook/legacy-hook-compile-resource";
import * as common_const_js_1 from "../../../const/common-const";
import * as ohos_logger_js_1 from "../../../utils/log/ohos-logger";
import * as task_names_js_1 from "../../common/task-names";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyProcessHarArtifacts extends abstract_process_har_artifacts_js_1.AbstractProcessHarArtifacts {
  constructor(e) {
    super(e, LegacyFATask.PROCESS_HAR_ARTIFACTS), this.logger = ohos_logger_js_1.OhosLogger.getLogger(LegacyProcessHarArtifacts.name);
  }
  initTaskDepends() {
    this.declareDepends(cache_native_libs_js_1.CacheNativeLibs.name), this.declareDepends(legacy_hook_compile_resource_js_1.LegacyHookCompileResource.name), this.declareDepends(common_const_js_1.ArktSTaskConst.LINT_ARKTS);
  }
  copyCompiledSourceFileToTempDir() {
    throw new Error("Har does not support compiled source and package,which api type is FA");
  }
  copyDeclareFileToTempDir() {
    throw new Error("Har does not support compiled source and package,which api type is FA");
  }
  copyOtherGenerateFiles() {
    if (this._harExtendInfo.isObfuscatedHar()) throw new Error("Har does not support package generate files which api type is FA");
    try {
      fs_extra_1.copySync(path_1.resolve(this.generateDir, this.isOhpmProject ? common_const_js_1.CommonConst.OH_PACKAGE_JSON5 : common_const_js_1.CommonConst.PACKAGE_JSON), path_1.resolve(this.taskTmpDir, this.isOhpmProject ? common_const_js_1.CommonConst.OH_PACKAGE_JSON5 : common_const_js_1.CommonConst.PACKAGE_JSON));
    } catch (e) {
      this.logger.warn("Failed to copy the temporary path when packing the HAR: ", e.message);
    }
  }
  processPackageJson() {}
  processSourceMaps() {}
  addPreloadSystemSoJson() {}
}
