/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/hsp-target-utils.js
 */
import os_1 from "os";
import path_1 from "path";
import * as task_util_1 from "@ohos/hvigor/src/base/util/task-util";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_plugin_id_js_1 from "../plugin/common/ohos-plugin-id";
import * as module_task_service_js_1 from "../tasks/service/module-task-service";
import * as ohos_logger_js_1 from "./log/ohos-logger";
import * as target_utils_js_1 from "./target-utils";
export class HspTargetUtils {
  static calDepHspTargets(e) {
    const t = new Map();
    return e.getModuleService().getHspModuleDependencyNames().forEach(o => {
      this.getHspTargetDatas(e, o).forEach(e => t.set(o, e));
    }), t;
  }
  static getHspTargetDatas(e, t) {
    var o;
    const s = e.getTargetData(),
      r = e.getModuleService(),
      a = r.getModuleModel(),
      g = a.getName(),
      l = null !== (o = (0, task_util_1.getAlignTarget)()) && undefined !== o ? o : s.getTargetName(),
      i = s.getProduct().name,
      u = a.getModule().findModuleByName(t);
    u || this._log.printErrorExit("HSP_NOT_FOUND", [t], [[t]]);
    const _ = () => {
        const e = `Module '${g}@${l}' depends on shared library module '${t}', `,
          o = `but neither target '${l}' nor target '${common_const_js_1.DefaultTargetConst.DEFAULT_TARGET}' of module '${t}' is applied to current product '${i}'.${os_1.EOL}`,
          r = `but target '${l}' of module '${t}' is not applied to current product '${i}'.${os_1.EOL}`,
          a = `\tSolution: Apply one of these targets of module '${t}' to product '${i}': ${os_1.EOL}\t\t- default`,
          u = `${os_1.EOL}\t\t- ${l}`,
          _ = l === common_const_js_1.DefaultTargetConst.DEFAULT_TARGET ? `${e}${r}${a}` : `${e}${o}${a}${u}`;
        this._log.printErrorExit("HSP_TARGET_NOT_MATCH", [s.getTargetName(), t, _], [[t, g]]);
      },
      n = u.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN);
    if (!n) {
      const e = r.getHspModuleDependencies().filter(e => e[0] === t);
      this._log.printErrorExit("MODULE_PLUGIN_NOT_RECEIVE", [t, path_1.resolve(e[0][1].getDependencyRootPath(), "hvigorfile.ts")], [[t], [t]]);
    }
    const c = n.getNeedExecTargetServiceList();
    0 === c.length && _();
    const d = target_utils_js_1.TargetUtils.pushTryTargets({
        projectModel: e.getModuleService().getProjectModel(),
        moduleName: t,
        superTargetName: s.getTargetName()
      }),
      p = [];
    for (const e of d) {
      const t = c.find(t => t.getTargetData().getTargetName() === e && (0, module_task_service_js_1.checkHasTargetApplyProduct)(t.getTargetData().getModuleModel(), e, i));
      if (undefined !== t) {
        p.push(t.getTargetData());
        break;
      }
    }
    if (0 === p.length && c.length > 0) {
      const e = c.find(e => (0, module_task_service_js_1.checkHasTargetApplyProduct)(e.getModuleService().getModuleModel(), e.getTargetData().getTargetName(), i));
      e ? p.push(e.getTargetData()) : _();
    }
    return p;
  }
  static calHspTargets(e, t) {
    const o = new Map();
    return t.forEach(t => {
      this.getHspTargetDatas(e, t).forEach(e => o.set(t, e));
    }), o;
  }
}
HspTargetUtils._log = ohos_logger_js_1.OhosLogger.getLogger("HspTargetUtils");
