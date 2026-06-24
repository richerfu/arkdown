/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/plugin-inspector.js
 */
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class PluginInspector {
  static checkFeatureEntryModules(e) {
    const o = e.getModuleModel();
    if (o.getModuleType() !== module_type_enum_js_1.ModuleType.Feature) return;
    const t = o.getProfileOpt().entryModules;
    if (undefined === t) return;
    const r = e.getProjectModel().getAllEntryModules();
    t.every(e => r.has(e)) || this._log.printErrorExit("ERROR_ENTRY_CONFIG", [], [[o.getName(), o.getProfilePath()]]);
  }
  static checkAtomicServiceHspModules(e) {
    var o;
    const t = e.getModuleModel(),
      r = null === (o = e.getTaskService()) || undefined === o ? undefined : o.hasHspDependencies();
    (null == t ? undefined : t.isInstallFree()) && r && this._log.printErrorExit("ATOMIC_SERVICE_RELY_HSP", [null == t ? undefined : t.getName()]);
  }
}
PluginInspector._log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-plugin-factory");
