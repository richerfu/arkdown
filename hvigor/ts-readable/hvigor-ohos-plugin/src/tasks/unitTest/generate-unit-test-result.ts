/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/generate-unit-test-result.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import * as common_const_js_1 from "../../const/common-const";
import * as hmos_sdk_loader_js_1 from "../../sdk/hmos-sdk-loader";
import * as ohos_sdk_loader_js_1 from "../../sdk/ohos-sdk-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as device_coverage_queue_center_js_1 from "../device-coverage-queue-center";
import * as ohos_module_task_js_1 from "../task/ohos-module-task";
export class GenerateUnitTestResult extends ohos_module_task_js_1.OhosModuleTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.GENERATE_UNIT_TEST_RESULT), this.logger = ohos_logger_js_1.OhosLogger.getLogger(GenerateUnitTestResult.name);
  }
  async getHmosPreviewerPath() {
    var e;
    const t = hmos_sdk_loader_js_1.HmosSdkLoader.getInstance(),
      {
        compileSdkVersion: o
      } = this.targetData.getApiMeta();
    return null === (e = (await t.getHmosSdkComponents(o, [sdkmanager_common_1.ComponentPath.PREVIEWER])).get(sdkmanager_common_1.ComponentPath.PREVIEWER)) || undefined === e ? undefined : e.getLocation();
  }
  async getHmsPreviewerPath() {
    var e;
    const t = hmos_sdk_loader_js_1.HmosSdkLoader.getInstance(),
      {
        compileSdkVersion: o
      } = this.targetData.getApiMeta();
    return null === (e = (await t.getHmsSdkComponents(o, [sdkmanager_common_1.ComponentPath.PREVIEWER])).get(sdkmanager_common_1.ComponentPath.PREVIEWER)) || undefined === e ? undefined : e.getLocation();
  }
  async getOhosPreviewerPath() {
    const e = ohos_sdk_loader_js_1.OhosSdkLoader.getInstance(),
      {
        compileSdkVersion: t
      } = this.targetData.getApiMeta(),
      o = [...(await e.getOhosSdkComponents(t, [sdkmanager_common_1.ComponentPath.PREVIEWER])).values()][0];
    return null == o ? undefined : o.getLocation();
  }
  getPreviewerPath() {
    return this.targetData.isHarmonyOS() ? this.getHmosPreviewerPath() : this.getOhosPreviewerPath();
  }
  async doTaskAction() {
    await device_coverage_queue_center_js_1.deviceCoverageQueueCenter.executeProxy({
      logger: this.logger,
      sdkApiPath: (await this.getPreviewerPath()) || "",
      hmsSdkApiPath: (this.targetData.isHarmonyOS() ? await this.getHmsPreviewerPath() : "") || "",
      runtimeType: this.targetData.isHarmonyOS() ? "HarmonyOS" : "OpenHarmony",
      modulePath: this.pathInfo.getModulePath(),
      projectPath: this.projectModel.getProjectDir(),
      coverage: "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.OHOS_TEST_COVERAGE),
      hspDependencies: this.service.getHspDependencyPaths()
    }, "generateUnitTestResult");
  }
  initTaskDepends() {}
}
