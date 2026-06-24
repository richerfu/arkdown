/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/cache-native-libs.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as abstract_native_strip_js_1 from "./abstract/abstract-native-strip";
import * as task_names_js_1 from "./common/task-names";
import * as do_native_strip_js_1 from "./do-native-strip";
import * as cache_libs_js_1 from "./worker/cache-libs";
var __decorate = this && this.__decorate || function (e, t, i, s) {
  var a,
    r = arguments.length,
    o = r < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, i) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) o = Reflect.decorate(e, t, i, s);else for (var c = e.length - 1; c >= 0; c--) (a = e[c]) && (o = (r < 3 ? a(o) : r > 3 ? a(t, i, o) : a(t, i)) || o);
  return r > 3 && o && Object.defineProperty(t, i, o), o;
};
export class CacheNativeLibs extends abstract_native_strip_js_1.AbstractNativeStrip {
  declareInputs() {
    const e = this.isBinxoValid();
    return super.declareInputs().set("isBinxoValid", e);
  }
  get cacheStrippedNativeLibs() {
    return this.strippedNativeLibs;
  }
  get cacheBinxoNativeLibs() {
    return this.doNativeBinxoLibs;
  }
  get buildCacheFilePath() {
    return this.cacheFilePath;
  }
  get binxoCacheFilePath() {
    return this.cacheBinxoFilePath;
  }
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.CACHE_NATIVE_LIBS), this.logger = ohos_logger_js_1.OhosLogger.getLogger(CacheNativeLibs.name);
  }
  async doTaskAction() {
    var e;
    const t = path_1.resolve(__dirname, "./worker/cache-libs.js/cacheNativeLibs"),
      i = this.isBinxoValid(),
      s = i ? this.cacheBinxoNativeLibs : this.intermediatesProcessLibs,
      a = {
        enableAsanBinxo: i,
        excludeSoFromBinxo: null !== (e = this.excludeSoFromBinxo) && undefined !== e ? e : []
      },
      r = {
        workInput: {
          intermediatesProcessLibs: this.intermediatesProcessLibs,
          binxoNativeLibs: this.cacheBinxoNativeLibs,
          strippedNativeLibs: this.cacheStrippedNativeLibs,
          inputCacheStripLibs: s,
          cacheStripFilePath: this.buildCacheFilePath,
          cacheBinxoFilePath: this.binxoCacheFilePath,
          debugSymbol: this.debugSymbol,
          binxoSymbol: a
        }
      };
    if (this.getWorkerPool().submit(this, t, r).getState() === hvigor_1.TaskState.REJECT) {
      const e = "cache native libs",
        t = this.durationEvent.createSubEvent(e, "");
      t.start(), this.logger.debug("Worker dispatch failed, running cacheNativeLibs on main thread."), await (0, cache_libs_js_1.cacheNativeLibs)(r.workInput), t.stop(), t.setLog(e, hvigor_1.MetricLogType.INFO);
    }
  }
  initTaskDepends() {
    this.declareDepends(do_native_strip_js_1.DoNativeStrip.name);
  }
}
__decorate([(0, hvigor_1.InputFile)({
  isDirectory: true
})], CacheNativeLibs.prototype, "cacheStrippedNativeLibs", null), __decorate([(0, hvigor_1.InputFile)({
  isDirectory: true
})], CacheNativeLibs.prototype, "cacheBinxoNativeLibs", null), __decorate([(0, hvigor_1.OutputFile)({
  isDirectory: false
})], CacheNativeLibs.prototype, "buildCacheFilePath", null), __decorate([(0, hvigor_1.OutputFile)({
  isDirectory: false
})], CacheNativeLibs.prototype, "binxoCacheFilePath", null);
