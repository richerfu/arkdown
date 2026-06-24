/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/process-project-privacy-profile.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../../const/common-const";
import * as abstract_hap_module_plugin_js_1 from "../../plugin/common/abstract-hap-module-plugin";
import * as ohos_plugin_id_js_1 from "../../plugin/common/ohos-plugin-id";
import * as hsp_plugin_js_1 from "../../plugin/hsp-plugin";
import * as json_util_js_1 from "../../utils/json-util";
import * as task_names_js_1 from "../common/task-names";
import * as make_project_pack_info_js_1 from "../make-project-pack-info";
import * as ohos_app_task_js_1 from "../task/ohos-app-task";
import * as merge_pac_files_js_1 from "./merge-pac-files";
import * as pac_util_js_1 from "./pac-util";
import * as trace_pac_js_1 from "./trace-pac";
var Task = task_names_js_1.TaskNames.Task;
export class ProcessProjectPrivacyProfile extends ohos_app_task_js_1.OhosAppTask {
  constructor(e, s) {
    super(e, s, Task.PROCESS_PROJECT_PRIVACY_PROFILE), this.outputPath = path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), common_const_js_1.CommonConst.PAC_ONLY_JSON), this.appPacJson = path_1.resolve(this.service.getProjectModel().getProjectDir(), common_const_js_1.CommonConst.APP_SCOPE, common_const_js_1.CommonConst.PAC_JSON);
  }
  declareInputs() {
    var e, s, t;
    const o = super.declareInputs();
    this.project.getAllSubModules().forEach(e => {
      var s, t, n;
      const i = json_util_js_1.JsonUtil.getJson5Obj(path_1.resolve(e.getNodeDir(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5));
      o.set(`${e.getName()}dependencies`, null !== (s = JSON.stringify(null == i ? undefined : i.dependencies)) && undefined !== s ? s : ""), o.set(`${e.getName()}devDependencies`, null !== (t = JSON.stringify(null == i ? undefined : i.devDependencies)) && undefined !== t ? t : ""), o.set(`${e.getName()}dynamicDependencies`, null !== (n = JSON.stringify(null == i ? undefined : i.dynamicDependencies)) && undefined !== n ? n : "");
    });
    const n = json_util_js_1.JsonUtil.getJson5Obj(path_1.resolve(this.project.getNodeDir(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5));
    return o.set("dependencies", null !== (e = JSON.stringify(null == n ? undefined : n.dependencies)) && undefined !== e ? e : ""), o.set("devDependencies", null !== (s = JSON.stringify(null == n ? undefined : n.devDependencies)) && undefined !== s ? s : ""), o.set("dynamicDependencies", null !== (t = JSON.stringify(null == n ? undefined : n.dynamicDependencies)) && undefined !== t ? t : ""), o;
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return fs_extra_1.existsSync(this.appPacJson) && e.addEntry(this.appPacJson), this.project.getAllSubModules().forEach(s => {
      const t = path_1.resolve(s.getNodeDir(), common_const_js_1.CommonConst.PAC_JSON);
      !s.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN) && fs_extra_1.existsSync(t) && fs_extra_1.existsSync(t) && e.addEntry(t);
    }), e;
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet();
    return e.addEntry(this.outputPath), e;
  }
  initTaskDepends() {
    this.dependsOn(make_project_pack_info_js_1.MakeProjectPackInfo.name);
  }
  doTaskAction() {
    if (!fs_extra_1.existsSync(this.service.getSdkInfo().getPacSchema())) return;
    const e = [];
    this.processValidPacFiles(this.appPacJson, e), this.service.getAllModuleOrProjectDependencies().forEach(s => {
      const t = path_1.resolve(s.getDependencyRootPath(), common_const_js_1.CommonConst.PAC_JSON);
      this.processValidPacFiles(t, e);
    }), this.project.getAllSubModules().forEach(s => {
      var t;
      const o = null !== (t = s.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN)) && undefined !== t ? t : s.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN);
      if (undefined !== o && (o instanceof abstract_hap_module_plugin_js_1.AbstractHapModulePlugin || o instanceof hsp_plugin_js_1.HspPlugin) && o.getTaskService()) {
        const t = o.getTaskService();
        if (!s.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN)) {
          const t = path_1.resolve(s.getNodeDir(), common_const_js_1.CommonConst.PAC_JSON);
          this.processValidPacFiles(t, e);
        }
        t.getAllModuleOrProjectDependencies().forEach(s => {
          const t = path_1.resolve(s.getDependencyRootPath(), common_const_js_1.CommonConst.PAC_JSON);
          this.processValidPacFiles(t, e);
        });
      }
    });
    const s = e.map(e => (0, pac_util_js_1.checkPacJsonExistAndGetObj)(e)).filter(e => undefined !== e),
      t = (0, merge_pac_files_js_1.mergePacFiles)(s);
    fs_extra_1.outputJSONSync(this.outputPath, t), (0, trace_pac_js_1.tracePac)(this.project.getName());
  }
  processValidPacFiles(e, s) {
    (0, pac_util_js_1.pacFileValidate)(this.project.getName(), e, this.service.getSdkInfo().getPacSchema()) && s.push(e);
  }
}
