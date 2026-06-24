/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/hooks/module-incremental-skip-optimization.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import fs_1 from "fs";
import path_1 from "path";
export class ModuleIncrementalSkipService {
  constructor() {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(ModuleIncrementalSkipService.name);
  }
  processModuleSkip(e) {
    const t = path_1.resolve(hvigor_1.PathUtil.getHvigorCacheDir(), "cache", "changed_modules.txt");
    !fs_1.existsSync(t) || inject_util_js_1.InjectUtil.isPreviewProcess() || inject_util_js_1.InjectUtil.isUnitTestProcess() ? this._log.warn("Incremental skip feature will be disabled, since record file not exist or in unsupported build mode (preview and unit test).") : hvigor_1.coreParameter.startParams.incrementalExecution ? (this.getModuleDependMap(e), this.getBuildModuleStat(e) ? this.readChangedFile(t) : this._log.warn("Module status not enough, the module level incremental skip feature will be disabled.")) : this._log.warn("Since incremental mode has been closed, the module level incremental skip feature will be disabled.");
  }
  getBuildModuleStat(e) {
    var t, i;
    let o = [],
      l = new Set();
    const r = hvigor_1.hvigorCore.getExtraConfig().get("module");
    if (undefined === r) e.getAllSubModules().forEach(e => {
      l.add(e.getName());
    });else for (const t of r.split(",")) {
      const i = t.split("@")[0],
        l = e.findModuleByName(i);
      if (undefined === l) return false;
      o.push(l);
    }
    for (const e of o) l.add(e.getName()), null === (t = hvigor_1.hvigorCore.getModuleDepRelationMap().get(e.getName())) || undefined === t || t.forEach(e => {
      l.add(e);
    });
    for (const t of l) {
      const o = null === (i = e.findModuleByName(t)) || undefined === i ? undefined : i.getNodeDir();
      if (!o) return false;
      if (!fs_1.existsSync(path_1.resolve(o, "build"))) return this._log.warn(`When using module incremental skip feature, failed to find build result in module: ${t}`), false;
    }
    return true;
  }
  getModuleDependMap(e) {
    e.getAllSubModules().forEach(e => {
      const t = new Set();
      e.getAllPluginIds().forEach(i => {
        var o, l, r;
        const n = e.getPluginById(i);
        Array.from(null !== (r = null === (l = null === (o = null == n ? undefined : n.getTaskService()) || undefined === o ? undefined : o.getDependencyInfo()) || undefined === l ? undefined : l.getModuleDependencyMap().keys()) && undefined !== r ? r : []).forEach(e => {
          t.has(e) || t.add(e);
        });
      }), hvigor_1.hvigorCore.getModuleDepRelationMap().set(e.getName(), t);
    });
  }
  readChangedFile(e) {
    const t = fs_1.readFileSync(e, "utf8").split("\n").map(e => e.trim()).filter(e => "" !== e);
    hvigor_1.hvigorCore.setChangedModuleNameList(new Set(t));
  }
}
