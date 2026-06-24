/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hot-reload-ark-compile.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as fse from "fs-extra";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ark_compile_js_1 from "./ark-compile";
export class HotReloadArkCompile extends ark_compile_js_1.ArkCompile {
  constructor() {
    super(...arguments), this.logger = ohos_logger_js_1.OhosLogger.getLogger(HotReloadArkCompile.name);
  }
  async doTaskAction() {
    const e = this.pathInfo.getBuildConfigPath();
    let t, o;
    if (fse.existsSync(e)) {
      const i = fse.readJsonSync(e);
      t = i.compileConfig, o = i.patchConfig;
    } else this.logger.debug(`Build config file not found: ${e}`);
    this.allowToPreloadSystemSo() && this.addPreloadSystemSoFileList();
    const i = {
      ...(await this.initDefaultArkCompileConfig()),
      ...(null != t ? t : {}),
      cachePath: this.getTaskTempDir(this.targetData)
    };
    if (i.obfuscationOptions && delete i.obfuscationOptions, i.aceModuleJsonPath = this.aceModuleJsonPath, i.watchMode = "true", i.projectPath = i.aceModuleRoot, i.transformAbc = !!this.getTransformAbcAction(), fse.ensureDirSync(i.cachePath), fse.ensureDirSync(i.aceModuleBuild), this._log.debug(`build config: ${i}`), o && i.aceBuildJson && fse.existsSync(i.aceBuildJson)) {
      const e = fse.readJsonSync(i.aceBuildJson);
      e.patchConfig = o, fse.outputJSONSync(i.aceBuildJson, e);
    }
    const r = hvigor_1.watchWorker.createWorker(this, path_1.resolve(__dirname, "./worker/hot-reload-build.js"), true, e => this.handleCompileEvents(e, this.getSubDurationEvent(r), true), {
      workerData: {
        projectConfig: i,
        logLevel: this.logger.getLevel().levelStr
      }
    }, {
      customType: hvigor_arkts_compose_1.TRANSFORM_ABC_WORKER_REQUEST,
      action: this.doBeforeObfuscationAction.bind(this)
    });
  }
  initTaskDepends() {
    this.dependsOnHook("buildHotReloadResource");
  }
}
