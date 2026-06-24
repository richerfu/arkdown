/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-metadata.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import * as common_const_js_1 from "../const/common-const";
import * as abstract_generate_metadata_js_1 from "./abstract/abstract-generate-metadata";
import * as task_names_js_1 from "./common/task-names";
import * as global_project_data_service_js_1 from "./service/global-project-data-service";
import * as sign_module_remote_hsp_js_1 from "./sign-module-remote-hsp";
var __decorate = this && this.__decorate || function (e, t, a, r) {
  var o,
    s = arguments.length,
    n = s < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, a) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(e, t, a, r);else for (var i = e.length - 1; i >= 0; i--) (o = e[i]) && (n = (s < 3 ? o(n) : s > 3 ? o(t, a, n) : o(t, a)) || n);
  return s > 3 && n && Object.defineProperty(t, a, n), n;
};
var Task = task_names_js_1.TaskNames.Task;
export class GenerateMetadata extends abstract_generate_metadata_js_1.AbstractGenerateMetadata {
  constructor(e) {
    super(e, Task.GENERATE_METADATA), this._hspDepsTarget = new Map();
  }
  get jsonPath() {
    return this.sourceSetModel.getModuleTargetRes().getJsonPath();
  }
  get relatedEntryJsonList() {
    return this.getRelatedEntryJsonList(common_const_js_1.CommonConst.MODULE_JSON5);
  }
  initTaskDependsForOtherModule() {
    this.service.getHspModuleDependencyNames().forEach(e => {
      var t, a, r;
      for (const o of [this.targetData.getTargetName(), common_const_js_1.DefaultTargetConst.DEFAULT_TARGET]) {
        const s = this.targetData.findTargetDataByName(e, o);
        if (undefined === s) continue;
        const n = `${null !== (t = s.getTargetName()) && undefined !== t ? t : common_const_js_1.DefaultTargetConst.DEFAULT_TARGET}@${Task.GENERATE_METADATA.name}`;
        if (null === (a = this.node.findModuleByName(e)) || undefined === a ? undefined : a.getTaskByName(n)) return this._hspDepsTarget.set(e, null !== (r = null == s ? undefined : s.getTargetName()) && undefined !== r ? r : common_const_js_1.DefaultTargetConst.DEFAULT_TARGET), void this.declareDepends(n, e);
      }
    });
  }
  getDistroFilterObj(e) {
    var t, a;
    return null !== (a = null === (t = null == e ? undefined : e.getTargetOpt().config) || undefined === t ? undefined : t.distroFilter) && undefined !== a ? a : undefined;
  }
  initTaskDepends() {
    const e = this.service.getProjectModel().getRemoteHspPath(),
      t = this.moduleModel.getRemoteHspPath(),
      a = global_project_data_service_js_1.GlobalProjectDataService.getInstance().getModuleRemoteHspPromise().has(this.moduleName);
    (fse.existsSync(e) || fse.existsSync(t)) && (a || (global_project_data_service_js_1.GlobalProjectDataService.getInstance().setModuleRemoteHspPromise(this.moduleName), this.dependsOn(`${this.targetData.getTargetName()}@${sign_module_remote_hsp_js_1.SignModuleRemoteHsp.name}`)));
  }
}
__decorate([(0, hvigor_1.InputFile)()], GenerateMetadata.prototype, "jsonPath", null), __decorate([(0, hvigor_1.InputFiles)()], GenerateMetadata.prototype, "relatedEntryJsonList", null);
