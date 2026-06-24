/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-har-js-manifest.js
 */
import * as path from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as fse from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as inject_const_js_1 from "../../const/inject-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as legacy_hook_compile_resource_js_1 from "./hook/legacy-hook-compile-resource";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
const PAGE = inject_const_js_1.InjectConst.PREVIEWER_REPLACE_PAGE;
export class LegacyGenerateHarJsManifest extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, LegacyFATask.GENERATE_HAR_JS_MANIFEST), this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyGenerateHarJsManifest.name), this.previewPage = hvigor_1.hvigorCore.getExtraConfig().get(PAGE), this._moduleModel = this.service.getModuleModel(), this._configOpt = (0, wdk_1.cloneDeep)(this._moduleModel.getJsonObjByTargetName(this.targetData.getTargetName())), this._minPlatformVersion = this.targetData.getCompatibleApiVersion();
  }
  initTaskDepends() {
    this.declareDepends(legacy_hook_compile_resource_js_1.LegacyHookCompileResource.name);
  }
  static getWindowObj() {
    return {
      autoDesignWidth: false,
      designWidth: 454
    };
  }
  declareInputs() {
    const e = new Map(),
      t = (0, hvigor_common_1.hash)(`${JSON.stringify(this._configOpt)}`);
    return e.set("CONFIG_OBJ", t), e.set("MIN_PLATFORM_VERSION", this._minPlatformVersion), this.previewPage && e.set("PREVIEW_PAGE", this.previewPage), e;
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntry(this.targetData.getPathInfo().getIntermediatesLegacyManifestJson());
  }
  doTaskAction() {
    this.generateJsonInfo(this._configOpt, this.pathInfo);
  }
  getManifestPages() {
    return this.previewPage ? (this._log.debug(`generate ${this.previewPage} for manifest.`), [this.previewPage]) : [];
  }
  generateJsonInfo(e, t) {
    const s = {
        appID: e.app.bundleName,
        versionName: e.app.version.name,
        versionCode: e.app.version.code,
        minPlatformVersion: this._minPlatformVersion,
        appName: "",
        deviceType: e.module.deviceType,
        window: LegacyGenerateHarJsManifest.getWindowObj(),
        pages: this.getManifestPages()
      },
      o = path.resolve(t.getIntermediatesLegacyManifestJson(), build_directory_const_js_1.BuildArtifactConst.LEGACY_MANIFEST_JSON);
    fse.outputJSONSync(o, s);
  }
}
