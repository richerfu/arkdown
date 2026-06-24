/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/trace/ohos-trace.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as common_const_js_1 from "../../const/common-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as inject_util_js_1 from "../../utils/inject-util";
class OhosTrace {
  constructor() {
    this.data = OhosTrace.getDefaultOhosTraceData();
  }
  initOhosTrace() {
    hvigor_1.hvigor.buildFinishedInternal(() => {
      ohosTrace.traceModuleIncremental(), ohosTrace.traceFullBuild(), ohosTrace.transmitDataToManager(), this.data = OhosTrace.getDefaultOhosTraceData();
    });
  }
  static getDefaultOhosTraceData() {
    return {
      MODULES: [],
      NATIVE_COMPILER: "Default",
      IS_FULL_BUILD: true,
      PACKAGE_JSON: []
    };
  }
  transmitDataToManager() {
    hvigor_1.TraceManager.trace(OhosTrace.TRACE_KEY, this.data, () => {
      var e, t;
      null === (t = null === (e = this.data) || undefined === e ? undefined : e.MODULES) || undefined === t || t.forEach(e => {
        delete e.RESTOOL_COMPRESSION, delete e.INCREMENTAL_TASKS;
      });
    });
  }
  traceBuildMode(e) {
    var t, o;
    const _ = null === (t = hvigor_1.globalData.cliEnv.configProps) || undefined === t ? undefined : t.get("prop");
    _ && (this.data.BUILD_MODE = null !== (o = _.get("buildMode")) && undefined !== o ? o : e);
  }
  traceNativeCompiler(e) {
    this.data.NATIVE_COMPILER = null != e ? e : "Default";
  }
  traceModules(e, t) {
    const o = hvigor_1.TraceManager.anonymize(e),
      _ = this.data.MODULES.filter(e => e.MODULE_NAME !== o),
      s = inject_util_js_1.InjectUtil.getEntryModulesName().includes(e);
    _.push({
      MODULE_NAME: o,
      API_TYPE: t,
      INCLUDE_IN_BUILD: false,
      IS_COMMAND_LINE_ENTRY_MODULE: s,
      BUILD_OPTION: [],
      PACKAGE_JSON: []
    }), this.data.MODULES = _;
  }
  traceModuleType(e, t) {
    const o = this.getModule(e);
    o && (o.MODULE_TYPE = t);
  }
  getModule(e) {
    if (!e) return;
    const t = hvigor_1.TraceManager.anonymize(e);
    return this.data.MODULES.find(e => e.MODULE_NAME === t);
  }
  traceRestoolCompression(e, t) {
    const o = this.getModule(e);
    o && (o.RESTOOL_COMPRESSION = t);
  }
  traceIncrement(e, t, o) {
    var _;
    const s = this.getModule(e);
    s && (s.INCREMENTAL_TASKS = null !== (_ = s.INCREMENTAL_TASKS) && undefined !== _ ? _ : {}, s.INCREMENTAL_TASKS[t] = o);
  }
  traceIncludeInBuild(e, t) {
    const o = this.getModule(e);
    o && (o.INCLUDE_IN_BUILD = t);
  }
  traceIsUseCompilePlugin(e) {
    const t = this.getModule(e);
    t && (t.IS_USE_COMPILE_PLUGIN = true);
  }
  traceIsUseTransformLib(e) {
    const t = this.getModule(e);
    t && (t.IS_USE_TRANSFORMLIB = true);
  }
  traceByteCodeHar(e) {
    const t = this.getModule(e);
    t && (t.IS_BYTE_CODE_HAR = true);
  }
  traceUseNormalizedOHMUrl(e) {
    this.data.USE_NORMALIZED_OHMURL = e;
  }
  traceIsAutoLazyImport(e, t) {
    const o = this.getModule(e);
    o && (o.IS_AUTO_LAZY_IMPORT = t);
  }
  traceIsUsePacJson(e) {
    const t = this.getModule(e);
    t && (t.IS_USE_PAC = true);
  }
  traceBuildOption(e, t) {
    const o = this.getModule(e);
    o && (o.BUILD_OPTION = new hvigor_common_1.JsonTrace(common_const_js_1.TelemetryFileConst.BUILD_OPTION_TELEMETRY_PATH).getRecord(t));
  }
  tracePackageJson(e, t) {
    const o = this.getModule(e);
    o && (o.PACKAGE_JSON = new hvigor_common_1.JsonTrace(common_const_js_1.TelemetryFileConst.PACKAGE_JSON_TELEMETRY_PATH).getRecord(t));
  }
  traceProjectPackageJson(e) {
    this.data.PACKAGE_JSON = new hvigor_common_1.JsonTrace(common_const_js_1.TelemetryFileConst.PACKAGE_JSON_TELEMETRY_PATH).getRecord(e);
  }
  traceModuleIncremental() {
    for (const e of this.data.MODULES) {
      const t = e.INCREMENTAL_TASKS;
      if (!e.INCLUDE_IN_BUILD) {
        e.IS_INCREMENTAL_MODULE = false;
        continue;
      }
      if (!t) {
        e.IS_INCREMENTAL_MODULE = true;
        continue;
      }
      let o = true;
      Object.keys(t).forEach(e => {
        t[e] || (o = false);
      }), e.IS_INCREMENTAL_MODULE = o;
    }
  }
  traceFullBuild() {
    this.data.IS_FULL_BUILD = !this.data.MODULES.some(e => this.isEntryModule(e) && e.IS_INCREMENTAL_MODULE);
  }
  isEntryModule(e) {
    if (e.IS_COMMAND_LINE_ENTRY_MODULE) return true;
    if (!e.INCLUDE_IN_BUILD) return false;
    const t = new Map([["assembleHap", [module_type_enum_js_1.ModuleType.Entry, module_type_enum_js_1.ModuleType.Feature]], ["assembleHsp", [module_type_enum_js_1.ModuleType.Shared]], ["assembleHar", [module_type_enum_js_1.ModuleType.Har]], ["assembleApp", [module_type_enum_js_1.ModuleType.Entry, module_type_enum_js_1.ModuleType.Feature, module_type_enum_js_1.ModuleType.Shared]]]);
    for (const [o, _] of t) if (hvigor_1.hvigorCore.isCommandEntryTask(o) && e.MODULE_TYPE && _.includes(e.MODULE_TYPE)) return true;
    return false;
  }
}
export const ohosTrace = new OhosTrace();
OhosTrace.TRACE_KEY = "HVIGOR_OHOS_PLUGIN";
