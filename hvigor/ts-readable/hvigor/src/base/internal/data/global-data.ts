/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/data/global-data.js
 */
import * as log4js_1 from "log4js";
import * as const_js_1 from "../../../common/const/const";
import * as hvigor_config_loader_js_1 from "../../../common/util/hvigor-config-loader";
import * as hvigor_trace_js_1 from "../../common/trace/hvigor-trace";
import * as hvigor_core_js_1 from "../../external/core/hvigor-core";
import * as default_configuration_js_1 from "../../log/default-configuration";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as hvigor_config_reader_js_1 from "../../util/hvigor-config-reader";
import * as json5_reader_js_1 from "../../util/json5-reader";
import * as global_core_parameters_js_1 from "./global-core-parameters";
var _a, _b;
class GlobalData {
  init(e, r) {
    this.cliEnv = e, this.cliOpts = r, this.buildId = createBuildId(), hvigor_trace_js_1.hvigorTrace.traceBuildId(this.buildId);
  }
  clean() {
    this.buildId = undefined;
  }
}
export const globalData = new GlobalData();
const defaultTestProps = {
    pageType: "page",
    product: "default",
    buildRoot: ".test",
    unitTestMode: "true",
    isLocalTest: "true",
    "ohos-test-coverage": "true",
    "unit.test.replace.page": "../../../.test/testability/pages/Index"
  },
  defaultDeviceTestProps = {
    product: "default",
    buildMode: "test",
    isOhosTest: "true",
    "ohos-test-coverage": "true"
  };
export function initStartData(e) {
  hvigor_config_loader_js_1.HvigorConfigLoader.init(e), hvigor_log_js_1.HvigorLogger.clean(), initExtParam(e), initParamProperties(), mergeHvigorConfig(), setTestDefaultParameters(e), setDeviceTestDefaultParameters(e), mergeCliConfig(e), initParamProperties(), checkHvigorConfigParameterFile(), traceHvigorConfigProperties(), traceExperimentFeature(e);
}
export function refreshDaemonProcessEnv(e) {
  [{
    name: "ENABLE_MODULE_SKIP",
    type: "boolean"
  }, {
    name: "ENABLE_CPP_FUNCTION_LEVEL_INCREMENTAL",
    type: "boolean"
  }, {
    name: "DEVECO_SDK_HOME",
    type: "string"
  }, {
    name: "OHOS_BASE_SDK_HOME",
    type: "string"
  }].forEach(({
    name: r,
    type: a
  }) => {
    var t;
    const o = null === (t = e.env) || undefined === t ? undefined : t[r];
    undefined !== o && (process.env[r] = "boolean" === a ? "true" === o ? "true" : "false" : o);
  });
}
function prepareCustomizedParamValue() {
  const e = hvigor_config_reader_js_1.HvigorConfigReader.getBuildParamFilePath();
  e && (global_core_parameters_js_1.coreParameter.startParams.customizedParam = json5_reader_js_1.Json5Reader.getJson5Obj(e));
}
function initExtParam(e) {
  if (!e) return;
  const r = new Map();
  undefined !== e.prop && [e.prop].flat(2).forEach(e => {
    var a;
    const t = e.split("=");
    r.set(t[0], null === (a = null == t ? undefined : t.splice(1)) || undefined === a ? undefined : a.join("="));
  }), global_core_parameters_js_1.coreParameter.extParams = Object.fromEntries(r.entries()), global_core_parameters_js_1.coreParameter.workspaceDir = startEnvironment.workspaceDir;
}
function initParamProperties() {
  const e = hvigor_config_loader_js_1.HvigorConfigLoader.getInstance();
  global_core_parameters_js_1.coreParameter.properties.hvigorPoolMaxSize = regularizeNumber(e.getPropertiesConfigValue(const_js_1.HVIGOR_POOL_MAX_SIZE)), global_core_parameters_js_1.coreParameter.properties.hvigorPoolMaxCoreSize = regularizeNumber(e.getPropertiesConfigValue(const_js_1.HVIGOR_POOL_MAX_CORE_SIZE)), global_core_parameters_js_1.coreParameter.properties.hvigorPoolCacheCapacity = regularizeNumber(e.getPropertiesConfigValue(const_js_1.HVIGOR_POOL_CACHE_CAPACITY)), global_core_parameters_js_1.coreParameter.properties.hvigorPoolCacheTtl = regularizeNumber(e.getPropertiesConfigValue(const_js_1.HVIGOR_POOL_CACHE_TTL)), global_core_parameters_js_1.coreParameter.properties.ohosArkCompileMaxSize = regularizeNumber(e.getPropertiesConfigValue(const_js_1.OHOS_ARK_COMPILE_MAX_SIZE)), global_core_parameters_js_1.coreParameter.properties.hvigorMemoryThreshold = regularizeNumber(e.getPropertiesConfigValue(const_js_1.HVIGOR_MEMORY_THRESHOLD)), prepareCustomizedParamValue();
}
function regularizeNumber(e) {
  if (!("string" == typeof e || undefined === e || e < 0)) return Math.floor(e);
}
function getPropMap(e) {
  const r = new Map();
  if (!e) return r;
  return ("string" == typeof e ? [e] : e).forEach(e => {
    const [a, t] = e.split("="),
      o = "coverage" === a ? "ohos-test-coverage" : a;
    r.set(o, t);
  }), r;
}
function setTestDefaultParameters(e) {
  if (!e) return;
  if (!e._.includes("test")) return;
  e.mode || (e.mode = "module");
  const r = getPropMap(e.prop);
  Object.keys(defaultTestProps).forEach(e => {
    r.has(e) || r.set(e, defaultTestProps[e]);
  });
  const a = [];
  r.forEach((e, r) => {
    a.push(`${r}=${e}`);
  }), e.prop = a;
}
function setDeviceTestDefaultParameters(e) {
  if (!e) return;
  if (!e._.includes("onDeviceTest")) return;
  e.mode || (e.mode = "module");
  const r = getPropMap(e.prop);
  Object.keys(defaultDeviceTestProps).forEach(e => {
    r.has(e) || r.set(e, defaultDeviceTestProps[e]);
  });
  const a = [];
  r.forEach((e, r) => {
    a.push(`${r}=${e}`);
  }), e.prop = a;
}
export function resetStartData() {
  startEnvironment = {
    ...defaultStartEnvironment
  }, global_core_parameters_js_1.coreParameter.clean();
}
function mergeHvigorConfig() {
  const e = hvigor_config_reader_js_1.HvigorConfigReader.getHvigorConfig();
  e ? (startEnvironment = {
    ...startEnvironment,
    ...e.environment
  }, global_core_parameters_js_1.coreParameter.properties = {
    ...global_core_parameters_js_1.defaultProperties,
    ...e.properties,
    ...global_core_parameters_js_1.coreParameter.properties
  }, setStartParameters(e)) : global_core_parameters_js_1.coreParameter.properties = {
    ...global_core_parameters_js_1.defaultProperties,
    ...global_core_parameters_js_1.coreParameter.properties
  };
}
function setStartParameters(e) {
  var r, a, t, o, s, i, l, _, n, c, m, g;
  global_core_parameters_js_1.coreParameter.startParams.incrementalExecution = null !== (a = null === (r = e.execution) || undefined === r ? undefined : r.incremental) && undefined !== a ? a : global_core_parameters_js_1.coreParameter.startParams.incrementalExecution, global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck = null !== (o = null === (t = e.execution) || undefined === t ? undefined : t.typeCheck) && undefined !== o ? o : global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck, global_core_parameters_js_1.coreParameter.startParams.parallelExecution = null !== (i = null === (s = e.execution) || undefined === s ? undefined : s.parallel) && undefined !== i ? i : global_core_parameters_js_1.coreParameter.startParams.parallelExecution, global_core_parameters_js_1.coreParameter.startParams.daemon = null !== (_ = null === (l = e.execution) || undefined === l ? undefined : l.daemon) && undefined !== _ ? _ : global_core_parameters_js_1.coreParameter.startParams.daemon, global_core_parameters_js_1.coreParameter.startParams.printStackTrace = null !== (c = null === (n = e.debugging) || undefined === n ? undefined : n.stacktrace) && undefined !== c ? c : global_core_parameters_js_1.coreParameter.startParams.printStackTrace, global_core_parameters_js_1.coreParameter.startParams.optimizationStrategy = null !== (g = null === (m = e.execution) || undefined === m ? undefined : m.optimizationStrategy) && undefined !== g ? g : global_core_parameters_js_1.coreParameter.startParams.optimizationStrategy, fillLoggingParameters(e);
}
function fillLoggingParameters(e) {
  var r, a, t;
  (null === (r = e.logging) || undefined === r ? undefined : r.level) && (global_core_parameters_js_1.coreParameter.startParams.logLevel = null !== (t = default_configuration_js_1.levelMap.get(null === (a = e.logging) || undefined === a ? undefined : a.level)) && undefined !== t ? t : global_core_parameters_js_1.coreParameter.startParams.logLevel);
}
function mergeCliConfig(e) {
  var r, a, t, o, s, i, l;
  const _ = null != e ? e : globalData.cliOpts;
  startEnvironment.nodeHome = null !== (r = _.nodeHome) && undefined !== r ? r : startEnvironment.nodeHome, global_core_parameters_js_1.coreParameter.startParams.hotCompile = _.hotCompile, global_core_parameters_js_1.coreParameter.startParams.hotReloadBuild = _.hotReloadBuild, global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck = null !== (a = _.enableBuildScriptTypeCheck) && undefined !== a ? a : global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck, global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck = null !== (t = _.typeCheck) && undefined !== t ? t : global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck, global_core_parameters_js_1.coreParameter.startParams.daemon = null !== (o = _.daemon) && undefined !== o ? o : global_core_parameters_js_1.coreParameter.startParams.daemon, global_core_parameters_js_1.coreParameter.startParams.printStackTrace = null !== (s = _.stacktrace) && undefined !== s ? s : global_core_parameters_js_1.coreParameter.startParams.printStackTrace, global_core_parameters_js_1.coreParameter.startParams.logLevel = _.debug ? log4js_1.levels.DEBUG : _.warn ? log4js_1.levels.WARN : _.error ? log4js_1.levels.ERROR : _.info ? log4js_1.levels.INFO : global_core_parameters_js_1.coreParameter.startParams.logLevel, global_core_parameters_js_1.coreParameter.startParams.parallelExecution = null !== (i = _.parallel) && undefined !== i ? i : global_core_parameters_js_1.coreParameter.startParams.parallelExecution, global_core_parameters_js_1.coreParameter.startParams.incrementalExecution = null !== (l = _.incremental) && undefined !== l ? l : global_core_parameters_js_1.coreParameter.startParams.incrementalExecution, global_core_parameters_js_1.coreParameter.startParams.optimizationStrategy = _.optimizationStrategy && Object.values(global_core_parameters_js_1.OptimizationStrategy).includes(_.optimizationStrategy) ? _.optimizationStrategy : global_core_parameters_js_1.coreParameter.startParams.optimizationStrategy, hvigor_trace_js_1.hvigorTrace.traceBaseConfig(global_core_parameters_js_1.coreParameter.startParams.incrementalExecution, global_core_parameters_js_1.coreParameter.startParams.daemon, global_core_parameters_js_1.coreParameter.startParams.parallelExecution, global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck), hvigor_trace_js_1.hvigorTrace.traceExecutionByOhpm("true" === hvigor_core_js_1.hvigorCore.getParameter().getExtParamInternal(const_js_1.ENFORCE_OHPM));
}
export const defaultStartEnvironment = {
  nodeHome: null !== (_a = process.env.NODE_HOME) && undefined !== _a ? _a : "",
  workspaceDir: null !== (_b = process.env.WORKSPACE_DIR) && undefined !== _b ? _b : ""
};
export const startEnvironment = {
  ...defaultStartEnvironment
};
let lastTimestamp = "",
  counter = 0,
  logger = hvigor_log_js_1.HvigorLogger.getLogger(GlobalData.name);
function createBuildId() {
  const e = new Date(),
    r = `${e.getFullYear()}${`0${e.getMonth() + 1}`.slice(-2)}${`0${e.getDate()}`.slice(-2)}${`0${e.getHours()}`.slice(-2)}${`0${e.getMinutes()}`.slice(-2)}${`0${e.getSeconds()}`.slice(-2)}${`00${e.getMilliseconds()}`.slice(-3)}`;
  return r !== lastTimestamp ? (lastTimestamp = r, counter = 0) : counter++, `${r}${counter}`;
}
function checkHvigorConfigParameterFile() {
  const e = hvigor_config_reader_js_1.HvigorConfigReader.getHvigorConfig(),
    r = null == e ? undefined : e.parameterFile;
  (null == r ? undefined : r.startsWith("/")) && logger.warn("The path of parameterFile started with '/' is not supported.");
}
function traceHvigorConfigProperties() {
  var e, r;
  const a = null !== (r = null === (e = hvigor_config_reader_js_1.HvigorConfigReader.getHvigorConfig()) || undefined === e ? undefined : e.properties) && undefined !== r ? r : {},
    t = Object.fromEntries(Object.entries(a).filter(([e, r]) => undefined !== r));
  hvigor_trace_js_1.hvigorTrace.traceConfigProperties(t);
}
function traceExperimentFeature(e) {
  [{
    name: "ENABLE_MODULE_SKIP",
    type: "boolean"
  }, {
    name: "ENABLE_CPP_FUNCTION_LEVEL_INCREMENTAL",
    type: "boolean"
  }].forEach(({
    name: r,
    type: a
  }) => {
    var t, o;
    const s = null !== (o = null === (t = e.env) || undefined === t ? undefined : t[r]) && undefined !== o ? o : "";
    "boolean" === a ? hvigor_trace_js_1.hvigorTrace.traceExperimentFeature(r, "true" === s) : "string" === a && hvigor_trace_js_1.hvigorTrace.traceExperimentFeature(r, s);
  });
}
