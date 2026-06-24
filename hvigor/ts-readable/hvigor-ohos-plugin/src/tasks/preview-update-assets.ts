/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/preview-update-assets.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
import * as res_model_loader_js_1 from "../utils/loader/file/res-model-loader";
var TASK = task_names_js_1.TaskNames.Task;
export class PreviewUpdateAssets extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, TASK.PREVIEW_UPDATE_ASSETS), this.logger = ohos_logger_js_1.OhosLogger.getLogger(PreviewUpdateAssets.name), this.paths = [], this.contents = [], this.buildConfigPath = this.pathInfo.getBuildConfigPath(true);
  }
  declareInputs() {
    const e = new Map();
    if (fs_extra_1.existsSync(this.buildConfigPath)) {
      const t = fs_extra_1.readJsonSync(this.buildConfigPath);
      t.port && delete t.port, e.set("previewBuildConfigJson", JSON.stringify(t));
    }
    return e;
  }
  declareOutputFiles() {
    if (fs_extra_1.existsSync(this.buildConfigPath)) {
      const e = fs_extra_1.readJsonSync(this.buildConfigPath).stageRouterConfig;
      if (!e) return new hvigor_1.FileSet();
      this.paths = e.paths, this.contents = e.contents, this.paths.length !== this.contents.length && this.logger._buildError("The number of 'paths' doesn't match that of 'contents'.")._file(this.buildConfigPath)._printErrorAndExit();
    } else this.logger.debug(`Build config file not found: ${this.buildConfigPath}`);
    return new hvigor_1.FileSet().addEntries(this.paths, {
      isDirectory: false
    });
  }
  doTaskAction() {
    const e = this.targetData.getModuleSourceSetModel().getModuleTargetRes().getJsonPath(),
      t = res_model_loader_js_1.resModelLoader.getModuleJson(e).module.pages;
    if (t) {
      const e = t.slice(t.indexOf(":") + 1);
      this.resolveRouterConfig(this.paths, this.contents, `${e}.json`);
    } else this.resolveRouterConfig(this.paths, this.contents, "main_pages.json");
  }
  resolveRouterConfig(e, t, s) {
    for (let o = 0; o < e.length; o++) {
      const i = e[o],
        r = t[o];
      i.endsWith("module.json") ? this.writeModuleJson(i, r) : i.endsWith(s) && this.writeMainPages(i, r);
    }
  }
  writeModuleJson(e, t) {
    if (this.moduleModel.isHarModule()) if (fs_extra_1.existsSync(e)) {
      const s = JSON.parse(t),
        o = fs_extra_1.readJsonSync(e);
      o.app && (s.app = o.app), s.module && (s.module.compileMode = this.targetCompileMode, s.module.packageName = this.packageJsonObj.name), fs_extra_1.writeJSONSync(e, s);
    } else fs_extra_1.ensureFileSync(e), fs_extra_1.writeFileSync(e, t);
  }
  writeMainPages(e, t) {
    let s;
    fs_extra_1.existsSync(e) ? s = fs_extra_1.readJsonSync(e) : (fs_extra_1.ensureFileSync(e), s = {
      src: []
    });
    const o = JSON.parse(t);
    s.src = o.src, fs_extra_1.writeJSONSync(e, s);
  }
  initTaskDepends() {
    this.dependsOnHook("buildPreviewerResource");
  }
}
