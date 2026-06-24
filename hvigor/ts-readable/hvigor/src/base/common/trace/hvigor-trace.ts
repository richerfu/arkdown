/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/hvigor-trace.js
 */
import * as trace_manager_js_1 from "./trace-manager";
class HvigorTrace {
  constructor() {
    this.configBlacklist = new Set(["hvigor.cacheDir", "ohos.buildDir", "ohos.arkCompile.sourceMapDir"]), this.data = {
      IS_INCREMENTAL: true,
      IS_DAEMON: true,
      IS_PARALLEL: true,
      IS_HVIGORFILE_TYPE_CHECK: false,
      IS_EXECUTION_OHPM_BY_HVIGOR: false,
      BUILD_ANALYSIS_MODE: "normal",
      TASK_TIME: {},
      CUSTOM_TASK_TIME: {},
      CUSTOM_PLUGIN_TIME: {},
      APIS: new Set(),
      CONFIG_EXPERIMENT: {}
    };
  }
  transmitDataToManager() {
    trace_manager_js_1.TraceManager.trace(HvigorTrace.TRACE_KEY, this.data, () => {
      delete this.data.BUILD_ID, delete this.data.ERROR_MESSAGE, this.data.TASK_TIME = {}, this.data.CUSTOM_TASK_TIME = {}, this.data.CUSTOM_PLUGIN_TIME = {}, this.data.APIS.clear();
    });
  }
  traceTotalTime(a) {
    this.data.TOTAL_TIME = a;
  }
  traceBaseConfig(a, t, e, r) {
    this.data.IS_INCREMENTAL = a, this.data.IS_DAEMON = t, this.data.IS_PARALLEL = e, this.data.IS_HVIGORFILE_TYPE_CHECK = r;
  }
  traceBuildAnalysisMode(a) {
    this.data.BUILD_ANALYSIS_MODE = "string" == typeof a ? a : false === a ? "false" : "normal";
  }
  traceExecutionByOhpm(a) {
    this.data.IS_EXECUTION_OHPM_BY_HVIGOR = a;
  }
  traceBuildId(a) {
    this.data.BUILD_ID = a;
  }
  traceCustomPluginTime(a, t) {
    let e;
    e = a ? trace_manager_js_1.TraceManager.anonymize(a) : `CustomPlugin${Object.keys(this.data.CUSTOM_PLUGIN_TIME).length}`, this.data.CUSTOM_PLUGIN_TIME[e] = t;
  }
  traceTaskTime(a, t, e) {
    var r, s;
    let i;
    i = "" === t ? "APP" : trace_manager_js_1.TraceManager.transformKey(trace_manager_js_1.TraceManager.anonymize(t));
    const _ = a.substring(a.indexOf("@") + 1),
      T = null !== (s = null === (r = this.data.TASK_TIME) || undefined === r ? undefined : r[i]) && undefined !== s ? s : {};
    T[_] = e, this.data.TASK_TIME && (this.data.TASK_TIME[i] = T);
  }
  traceCustomTaskTime(a, t, e) {
    var r, s;
    let i;
    i = "" === t ? "APP" : trace_manager_js_1.TraceManager.transformKey(trace_manager_js_1.TraceManager.anonymize(t));
    const _ = trace_manager_js_1.TraceManager.transformKey(trace_manager_js_1.TraceManager.anonymize(a)),
      T = null !== (s = null === (r = this.data.CUSTOM_TASK_TIME) || undefined === r ? undefined : r[i]) && undefined !== s ? s : {};
    T[_] = e, this.data.CUSTOM_TASK_TIME && (this.data.CUSTOM_TASK_TIME[i] = T);
  }
  traceErrorMessage(a) {
    var t, e;
    this.data.ERROR_MESSAGE = null !== (t = this.data.ERROR_MESSAGE) && undefined !== t ? t : [], this.data.ERROR_MESSAGE.push({
      CODE: a.code,
      MESSAGE: a.originMessage,
      SOLUTIONS: a.originSolutions,
      MORE_INFO: a.moreInfo,
      TIMESTAMP: null === (e = a.timestamp) || undefined === e ? undefined : e.getTime().toString(),
      COMPONENTS: a.components,
      CHECK_MESSAGE: a.checkMessage,
      STACK: a.stack,
      PROJECT_CONFIG: a.projectConfig
    });
  }
  insertUsedApi(a) {
    this.data.APIS.has(a) || this.data.APIS.add(a);
  }
  traceConfigProperties(a) {
    a = Object.entries(a).reduce((a, [t, e]) => {
      const r = trace_manager_js_1.TraceManager.transformKey(t);
      return e = this.configBlacklist.has(t) ? "" : e, a[r] = e, a;
    }, {}), this.data.CONFIG_PROPERTIES = a;
  }
  traceExperimentFeature(a, t) {
    this.data.CONFIG_EXPERIMENT || (this.data.CONFIG_EXPERIMENT = {}), this.data.CONFIG_EXPERIMENT[a] = t;
  }
}
export const hvigorTrace = new HvigorTrace();
HvigorTrace.TRACE_KEY = "HVIGOR";
