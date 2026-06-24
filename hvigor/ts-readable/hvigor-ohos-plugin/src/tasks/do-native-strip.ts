/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/do-native-strip.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as file_util_js_1 from "../utils/file-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as abstract_native_strip_js_1 from "./abstract/abstract-native-strip";
import * as task_names_js_1 from "./common/task-names";
import * as do_native_binxo_js_1 from "./do-native-binxo";
import * as process_libs_js_1 from "./process-libs";
import * as native_strip_js_1 from "./worker/native-strip";
var __decorate = this && this.__decorate || function (t, e, i, s) {
  var r,
    o = arguments.length,
    a = o < 3 ? e : null === s ? s = Object.getOwnPropertyDescriptor(e, i) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(t, e, i, s);else for (var n = t.length - 1; n >= 0; n--) (r = t[n]) && (a = (o < 3 ? r(a) : o > 3 ? r(e, i, a) : r(e, i)) || a);
  return o > 3 && a && Object.defineProperty(e, i, a), a;
};
export class DoNativeStrip extends abstract_native_strip_js_1.AbstractNativeStrip {
  declareInputs() {
    const t = this.isBinxoValid();
    return super.declareInputs().set("isBinxoValid", t);
  }
  get doBinxoNativeLibs() {
    return this.doNativeBinxoLibs;
  }
  get doStrippedNativeLibs() {
    return this.strippedNativeLibs;
  }
  constructor(t) {
    super(t, task_names_js_1.TaskNames.Task.DO_NATIVE_STRIP), this._log = ohos_logger_js_1.OhosLogger.getLogger(DoNativeStrip.name);
  }
  async doTaskAction() {
    var t, e, i, s;
    await fs_extra_1.ensureDir(this.doStrippedNativeLibs);
    const r = this.isBinxoValid() ? this.doBinxoNativeLibs : this.intermediatesProcessLibs;
    if ((0, native_strip_js_1.clearExpiredSoInStrippedDir)(this.doStrippedNativeLibs, r), await file_util_js_1.FileUtil.isEmptyDir(r)) return;
    const o = {
        debugSymbol: this.debugSymbol,
        strip: null === (e = null === (t = this.debugSymbol) || undefined === t ? undefined : t.strip) || undefined === e || e,
        exclude: null !== (i = this.exclude) && undefined !== i ? i : [],
        intermediatesProcessLibs: r,
        sdkLlvmStripPath: this.sdkInfo.getSdkLlvmStrip(),
        strippedNativeLibs: this.doStrippedNativeLibs,
        moduleName: this.moduleName,
        taskName: this.name,
        cacheFilePath: this.cacheFilePath,
        lastCache: path_1.resolve(this.pathInfo.getIntermediatesPatch(), "libs.json"),
        collectAllLibs: null === (s = this.targetService.getBuildOption().nativeLib) || undefined === s ? undefined : s.collectAllLibs
      },
      a = path_1.resolve(__dirname, "./worker/native-strip.js/strip"),
      n = {
        workInput: o
      };
    if (this.getWorkerPool().submit(this, a, n).getState() === hvigor_1.TaskState.REJECT) {
      const t = "strip native libs",
        e = this.durationEvent.createSubEvent(t, "");
      e.start(), this._log.debug("Worker dispatch failed, running native strip on main thread."), await (0, native_strip_js_1.strip)(o), e.stop(), e.setLog(t, hvigor_1.MetricLogType.INFO);
    }
  }
  initTaskDepends() {
    this.isBinxoValid() ? this.declareDepends(do_native_binxo_js_1.DoNativeBinxo.name) : this.declareDepends(process_libs_js_1.ProcessLibs.name);
  }
}
__decorate([(0, hvigor_1.InputFile)({
  isDirectory: true
})], DoNativeStrip.prototype, "doBinxoNativeLibs", null), __decorate([(0, hvigor_1.OutputFile)({
  isDirectory: true
})], DoNativeStrip.prototype, "doStrippedNativeLibs", null);
