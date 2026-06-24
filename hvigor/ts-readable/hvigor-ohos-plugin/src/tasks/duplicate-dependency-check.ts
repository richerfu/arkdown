/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/duplicate-dependency-check.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_app_task_js_1 from "./task/ohos-app-task";
import fs_extra_1 from "fs-extra";
import os_1 from "os";
import path_1 from "path";
import * as ohos_plugin_id_js_1 from "../plugin/common/ohos-plugin-id";
import * as har_plugin_js_1 from "../plugin/har-plugin";
import * as hsp_plugin_js_1 from "../plugin/hsp-plugin";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
const _log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-duplicateDependencyCheck");
export class DuplicateDependencyCheck extends ohos_app_task_js_1.OhosAppTask {
  constructor(e, s) {
    super(e, s, CommonTask.DUPLICATE_DEPENDENCY_CHECK), this.ohPackagePath = s.getProjectModel().getOhPackageJson5Path();
  }
  doTaskAction() {
    var e;
    if (!!!(null === (e = this.service.getBuildOption().strictMode) || undefined === e ? undefined : e.duplicateDependencyCheck)) return;
    const s = [],
      t = new Map(),
      o = new Map(),
      n = new Map(),
      i = new Map();
    this.collectProjectDependencies(n), this.project.getSubModules().forEach(e => {
      var s, t;
      const o = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAR_PLUGIN);
      if (o && o instanceof har_plugin_js_1.HarPlugin && (null === (s = o.getTaskService()) || undefined === s ? undefined : s.getModuleModel().isHarModule())) {
        const e = null === (t = o.getModuleModel()) || undefined === t ? undefined : t.getProjectDir();
        e && n.get(e) && i.set(e, n.get(e));
      }
    }), this.project.getSubModules().forEach(e => {
      const s = e.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN);
      if (!(s && s instanceof hsp_plugin_js_1.HspPlugin)) return;
      const n = s.getTaskService();
      if (!n) return;
      const a = e.getName();
      i.forEach((e, s) => {
        o.has(s) || o.set(s, []), o.get(s).push(a), t.set(s, e);
      }), n.getHarDependencies().forEach(e => {
        const s = e.getDependencyRootPath();
        o.has(s) || o.set(s, []), o.get(s).push(a), t.set(s, e.getPackageName());
      });
    }), o.forEach((e, o) => {
      e.length <= 1 || s.push(`[${e.join(", ")}] depend on the same HAR [${t.get(o)}].`);
    }), s.length && _log.printErrorExit("FOUND_HSPS_THAT_HAVE_THE_SAME_HAR_DEPENDENCIES", [s.length > 1 ? "dependencies" : "dependency", s.join(`${os_1.EOL}`)]);
  }
  collectProjectDependencies(e) {
    if (fs_extra_1.existsSync(this.ohPackagePath)) {
      const s = hvigor_1.ParseJsonFile.parseJsonFile(this.ohPackagePath);
      Object.keys(s.dependencies).forEach(t => {
        const o = s.dependencies[t],
          n = /^file:(.*)/i.test(o) ? /^file:(.*)/i.exec(o)[1] : o;
        if (!n) return;
        const i = path_1.resolve(this.project.getNodeDir(), n);
        fs_extra_1.existsSync(i) && e.set(i, t);
      });
    }
  }
  initTaskDepends() {
    this.dependsOn(CommonTask.PRE_BUILD_APP.name);
  }
}
