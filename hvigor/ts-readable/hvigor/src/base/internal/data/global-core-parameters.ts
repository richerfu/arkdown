/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/data/global-core-parameters.js
 */
import * as log4js_1 from "log4js";
export class CoreParameter {
  constructor() {
    this._properties = {}, this._extParams = {}, this._startParams = {
      ...defaultStartParam
    }, this._workspaceDir = "";
  }
  get properties() {
    return this._properties;
  }
  set properties(e) {
    this._properties = e;
  }
  get extParams() {
    return this._extParams;
  }
  set extParams(e) {
    this._extParams = e;
  }
  get startParams() {
    return this._startParams;
  }
  get workspaceDir() {
    return this._workspaceDir;
  }
  set workspaceDir(e) {
    this._workspaceDir = e;
  }
  clean() {
    this._properties = {}, this._extParams = {}, this._startParams = {
      ...defaultStartParam
    }, this._workspaceDir = "";
  }
}
export var AnalyzeMode, OptimizationStrategy;
export const AnalyzeModeMap = new Map([["default", AnalyzeMode.NORMAL], ["verbose", AnalyzeMode.ADVANCED], [false, AnalyzeMode.FALSE], ["false", AnalyzeMode.FALSE], ["normal", AnalyzeMode.NORMAL], ["advanced", AnalyzeMode.ADVANCED], ["ultrafine", AnalyzeMode.TRACE]]);
export const OldAnalyzeModeMap = new Map([["default", "normal"], ["verbose", "advanced"]]);
export const AnalyzeModeKeyMap = new Map([[AnalyzeMode.NORMAL, "normal"], [AnalyzeMode.ADVANCED, "advanced"], [AnalyzeMode.TRACE, "ultrafine"], [AnalyzeMode.FALSE, false]]);
export const LogLevelMap = new Map([["info", log4js_1.levels.INFO], ['debug"', log4js_1.levels.DEBUG], ['warn"', log4js_1.levels.WARN], ['error"', log4js_1.levels.ERROR]]);
export const defaultStartParam = {
  hvigorfileTypeCheck: false,
  parallelExecution: true,
  incrementalExecution: true,
  printStackTrace: false,
  daemon: true,
  analyze: AnalyzeMode.NORMAL,
  logLevel: log4js_1.levels.INFO,
  optimizationStrategy: OptimizationStrategy.MEMORY,
  hotCompile: false,
  hotReloadBuild: false,
  customizedParam: undefined
};
export const defaultProperties = {
  enableSignTask: true,
  skipNativeIncremental: false,
  "hvigor.keepDependency": true
};
export const coreParameter = new CoreParameter();
(function (e) {
  e[e.NORMAL = 0] = "NORMAL", e[e.ADVANCED = 1] = "ADVANCED", e[e.FALSE = 2] = "FALSE", e[e.TRACE = 3] = "TRACE";
})(AnalyzeMode = AnalyzeMode || (AnalyzeMode = {})), function (e) {
  e.PERFORMANCE = "performance", e.MEMORY = "memory";
}(OptimizationStrategy = OptimizationStrategy || (OptimizationStrategy = {}));
