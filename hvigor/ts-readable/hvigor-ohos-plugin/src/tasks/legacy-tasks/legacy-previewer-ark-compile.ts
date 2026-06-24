/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-previewer-ark-compile.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import path_1 from "path";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as legacy_compile_node_js_1 from "./legacy-compile-node";
export class LegacyPreviewerArkCompile extends legacy_compile_node_js_1.LegacyCompileNode {
  constructor() {
    super(...arguments), this.logger = ohos_logger_js_1.OhosLogger.getLogger(LegacyPreviewerArkCompile.name);
  }
  declareInputFiles() {
    return this.moduleModel.isHarModule() ? super.declareInputFiles().deleteEntry(this.aceBuildJsonDir) : super.declareInputFiles();
  }
  async doTaskAction() {
    const e = await this.getPreviewCompileConfig(this.logger);
    e.aceModuleJsonPath = undefined;
    const r = hvigor_1.watchWorker.createWorker(this, path_1.resolve(__dirname, "../worker/preview-build.js"), true, e => this.handleCompileEvents(e, this.getSubDurationEvent(r), true), {
      workerData: {
        projectConfig: e,
        logLevel: this.logger.getLevel().levelStr
      }
    });
  }
  initTaskDepends() {
    this.dependsOnHook("buildPreviewerResource");
  }
}
