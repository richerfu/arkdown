/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/do-native-binxo.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as file_util_js_1 from "../utils/file-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as abstract_native_strip_js_1 from "./abstract/abstract-native-strip";
import * as task_names_js_1 from "./common/task-names";
import * as process_libs_js_1 from "./process-libs";
import * as native_binxo_js_1 from "./worker/native-binxo";
import * as native_strip_js_1 from "./worker/native-strip";
var __decorate = this && this.__decorate || function (e, t, i, s) {
  var o,
    r = arguments.length,
    a = r < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, i) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, t, i, s);else for (var n = e.length - 1; n >= 0; n--) (o = e[n]) && (a = (r < 3 ? o(a) : r > 3 ? o(t, i, a) : o(t, i)) || a);
  return r > 3 && a && Object.defineProperty(t, i, a), a;
};
export class DoNativeBinxo extends abstract_native_strip_js_1.AbstractNativeStrip {
  declareInputs() {
    const e = this.isBinxoValid();
    return super.declareInputs().set("isBinxoValid", e);
  }
  get doBinxoNativeLibs() {
    return this.doNativeBinxoLibs;
  }
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.DO_NATIVE_BINXO), this._log = ohos_logger_js_1.OhosLogger.getLogger(DoNativeBinxo.name);
  }
  async doTaskAction() {
    var e, t;
    if (!this.isBinxoValid()) {
      const e = this.moduleModel.getModule().getBuildProfilePath();
      this._log.printErrorExit("DO_BINXO_MUST_OPEN_HWASAN", [e]);
    }
    if (await fs_extra_1.ensureDir(this.doBinxoNativeLibs), (0, native_strip_js_1.clearExpiredSoInStrippedDir)(this.doBinxoNativeLibs, this.intermediatesProcessLibs), await file_util_js_1.FileUtil.isEmptyDir(this.intermediatesProcessLibs)) return;
    const i = {
        excludeSoFromBinxo: null !== (e = this.excludeSoFromBinxo) && undefined !== e ? e : [],
        intermediatesLibs: this.intermediatesProcessLibs,
        sdkBinxoPath: this.sdkInfo.getSdkBinxoTool(),
        binxoNativeLibs: this.doBinxoNativeLibs,
        moduleName: this.moduleName,
        taskName: this.name,
        cacheFilePath: this.cacheBinxoFilePath,
        lastCache: path_1.resolve(this.pathInfo.getIntermediatesPatch(), "libs.json"),
        collectAllLibs: null === (t = this.targetService.getBuildOption().nativeLib) || undefined === t ? undefined : t.collectAllLibs
      },
      s = path_1.resolve(__dirname, "./worker/native-binxo.js/binxo"),
      o = {
        workInput: i
      };
    if (this.getWorkerPool().submit(this, s, o).getState() === hvigor_1.TaskState.REJECT) {
      const e = "binxo native libs",
        t = this.durationEvent.createSubEvent(e, "");
      t.start(), this._log.debug("Worker dispatch failed, running native binxo on main thread."), await (0, native_binxo_js_1.binxo)(i), t.stop(), t.setLog(e, hvigor_1.MetricLogType.INFO);
    }
  }
  initTaskDepends() {
    this.declareDepends(process_libs_js_1.ProcessLibs.name);
  }
}
__decorate([(0, hvigor_1.OutputFile)({
  isDirectory: true
})], DoNativeBinxo.prototype, "doBinxoNativeLibs", null);
