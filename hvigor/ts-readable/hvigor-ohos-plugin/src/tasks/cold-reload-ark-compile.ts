/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/cold-reload-ark-compile.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as fse from "fs-extra";
import * as common_const_js_1 from "../const/common-const";
import * as json_util_js_1 from "../utils/json-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ark_compile_js_1 from "./ark-compile";
import * as run_ark_js_1 from "./worker/run-ark";
import * as worker_id_helper_js_1 from "./worker/worker-id-helper";
export class ColdReloadArkCompile extends ark_compile_js_1.ArkCompile {
  constructor() {
    super(...arguments), this.logger = ohos_logger_js_1.OhosLogger.getLogger(ColdReloadArkCompile.name);
  }
  taskShouldDo() {
    const e = this.pathInfo.getBuildConfigPath();
    return super.taskShouldDo() && fse.existsSync(e);
  }
  declareInputFiles() {
    const e = super.declareInputFiles(),
      t = path_1.resolve(this.pathInfo.getIntermediatesPatch(), "changedFileList.json");
    return t && fse.existsSync(t) && e.addEntry(t), e;
  }
  async doTaskAction() {
    const e = this.pathInfo.getBuildConfigPath();
    let t;
    if (fse.existsSync(e)) {
      t = json_util_js_1.JsonUtil.getJson5Obj(e).compileConfig;
    } else this.logger.debug(`Build config file not found: ${e}`);
    const o = this.getColdReloadPatchConfig(),
      r = {
        ...(await this.initDefaultArkCompileConfig()),
        ...(null != t ? t : {})
      };
    if (r.obfuscationOptions && delete r.obfuscationOptions, r.aceModuleJsonPath = this.pathInfo.getIntermediatesArkModuleJsonPath(), r.watchMode = "false", r.projectPath = r.aceModuleRoot, r.collectImportersConfig && (r.collectImportersConfig.isReload = true), fse.ensureDirSync(r.cachePath), fse.ensureDirSync(r.aceModuleBuild), this._log.debug(`coldReload build config: ${r}`), o && r.aceBuildJson && fse.existsSync(r.aceBuildJson)) {
      const e = json_util_js_1.JsonUtil.getJson5Obj(r.aceBuildJson);
      e.patchConfig = o, fse.outputJSONSync(r.aceBuildJson, e);
    }
    let i = hvigor_1.coreParameter.properties.ohosArkCompileMaxSize;
    (undefined === i || i <= 0 || i > hvigor_1.PoolConstant.MAX_POOL_NUM) && (i = common_const_js_1.ArkPackConst.MAX_ARK_COMPILE_NUM);
    const s = this.getWorkerPool().getMaxPoolNum();
    let l = [...new Array(i).keys()];
    const n = (0, worker_id_helper_js_1.getWorkerIdWithModule)(r.modulePath);
    undefined !== n && s > 0 && (l = [(0, hvigor_common_1.mod)(n, Math.min(s, i))]);
    let a = null;
    a = await (0, run_ark_js_1.submitArkCompileWork)(this, this.getWorkerPool(), this.logger, r, (e, t) => {
      undefined !== t && (0, worker_id_helper_js_1.setWorkerIdWithModule)(r.modulePath, t), a && this.handleCompileEvents(e, this.getSubDurationEvent(a), true);
    }, l);
  }
  allowToPreloadSystemSo() {
    return false;
  }
  getColdReloadPatchConfig() {
    return {
      enableMap: "true",
      mode: "coldReload",
      oldMapFilePath: path_1.resolve(this.pathInfo.getInterMediatesLoaderOutPath(), "ets"),
      changedFileList: path_1.resolve(this.pathInfo.getIntermediatesPatch(), "changedFileList.json"),
      patchAbcPath: path_1.resolve(this.pathInfo.getIntermediatesPatch(), "ets"),
      removeChangedFileListInSdk: "true"
    };
  }
  initTaskDepends() {
    this.dependsOnHook("buildHotReloadResource");
  }
}
