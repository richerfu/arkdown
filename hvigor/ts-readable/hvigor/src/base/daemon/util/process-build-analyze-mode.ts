/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/util/process-build-analyze-mode.js
 */
import * as hvigor_config_loader_js_1 from "../../../common/util/hvigor-config-loader";
import * as global_core_parameters_js_1 from "../../internal/data/global-core-parameters";
import * as global_data_js_1 from "../../internal/data/global-data";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("process-build-analyze-mode");
export function processLegacyConfig(e) {
  processAnalyzeMode(e), processLegacyCliAnalyzeMode(e), processLegacyConfigAnalyzeMode(e);
}
function processAnalyzeMode(e) {
  const a = "--analyze=";
  if (!e) return;
  const o = e._.filter(e => e.startsWith(a));
  o && o.length && (e.analyze = o[o.length - 1].substring(10), e._ = e._.filter(e => !e.startsWith(a)));
}
function processLegacyConfigAnalyzeMode(e) {
  var a, o;
  const l = hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getHvigorConfig();
  if (undefined === (null === (a = null == l ? undefined : l.execution) || undefined === a ? undefined : a.analyze)) return;
  const r = global_core_parameters_js_1.OldAnalyzeModeMap.get(l.execution.analyze);
  r && _log.warn(`Build analysis mode '${l.execution.analyze}' is deprecated, please use '${r}' instead.`), (null == e ? undefined : e.analyze) || false === (null == e ? undefined : e.analyze) || (global_core_parameters_js_1.coreParameter.startParams.analyze = null !== (o = global_core_parameters_js_1.AnalyzeModeMap.get(l.execution.analyze)) && undefined !== o ? o : global_core_parameters_js_1.AnalyzeMode.NORMAL);
}
function processLegacyCliAnalyzeMode(e) {
  const a = null != e ? e : global_data_js_1.globalData.cliOpts;
  a.verboseAnalyze && (a.analyze = "advance", global_core_parameters_js_1.coreParameter.startParams.analyze = global_core_parameters_js_1.AnalyzeMode.ADVANCED, _log.warn("Command line '--verbose-analyze' is deprecated, please use '--analyze=advanced' instead.")), undefined !== a.analyze && ("boolean" == typeof a.analyze ? useOldAnalyzeMode(a.analyze) : useNewAnalyzeMode(a.analyze));
}
function useOldAnalyzeMode(e) {
  e ? (global_core_parameters_js_1.coreParameter.startParams.analyze = global_core_parameters_js_1.AnalyzeMode.NORMAL, _log.warn("Command line '--analyze' is deprecated, please use '--analyze=normal' instead.")) : (global_core_parameters_js_1.coreParameter.startParams.analyze = global_core_parameters_js_1.AnalyzeMode.FALSE, _log.warn("Command line '--no-analyze' is deprecated, please use '--analyze=false' instead."));
}
function useNewAnalyzeMode(e) {
  const a = global_core_parameters_js_1.AnalyzeModeMap.get(e);
  undefined === a ? _log.warn(`Build analysis mode: ${e} is not available, value should be one of: 'normal', 'advanced', 'ultrafine' and 'false'.`) : global_core_parameters_js_1.coreParameter.startParams.analyze = a;
}
