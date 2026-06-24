/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har-deduplicate/process-id-defined-file.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as json_util_js_1 from "../../utils/json-util";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
var __decorate = this && this.__decorate || function (e, t, s, i) {
  var o,
    r = arguments.length,
    a = r < 3 ? t : null === i ? i = Object.getOwnPropertyDescriptor(t, s) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, t, s, i);else for (var d = e.length - 1; d >= 0; d--) (o = e[d]) && (a = (r < 3 ? o(a) : r > 3 ? o(t, s, a) : o(t, s)) || a);
  return r > 3 && a && Object.defineProperty(t, s, a), a;
};
var Task = task_names_js_1.TaskNames.Task;
export class ProcessIdDefinedFile extends ohos_hap_task_js_1.OhosHapTask {
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.customIdDefinedFilePath && e.addEntry(this.customIdDefinedFilePath), e;
  }
  constructor(e) {
    super(e, Task.PROCESS_ID_DEFINED_FILE), this.customIdDefinedFilePath = this.getCustomIdDefinedFilePath(), this.idDefinedJsonFile = this.pathInfo.getIntermediatesIdDefinedJsonFile();
  }
  taskShouldDo() {
    return this.targetService.shouldDoDeduplicateTask() && undefined !== this.customIdDefinedFilePath && fs_extra_1.existsSync(this.customIdDefinedFilePath);
  }
  beforeTask() {
    super.beforeTask(), fs_extra_1.existsSync(this.idDefinedJsonFile) && fs_extra_1.rmSync(this.idDefinedJsonFile);
  }
  doTaskAction() {
    if (this.customIdDefinedFilePath) {
      const e = json_util_js_1.JsonUtil.getJson5Obj(this.customIdDefinedFilePath);
      fs_extra_1.outputJSONSync(this.idDefinedJsonFile, e, {
        spaces: "\t"
      });
    }
  }
  initTaskDepends() {}
  getCustomIdDefinedFilePath() {
    var e;
    const t = null === (e = this.targetService.getBuildOption().resOptions) || undefined === e ? undefined : e.idDefinedFilePath;
    if (t) {
      const e = path_1.normalize(t);
      return path_1.isAbsolute(e) ? e : path_1.resolve(this.projectModel.getProjectDir(), e);
    }
  }
}
__decorate([(0, hvigor_1.OutputFile)()], ProcessIdDefinedFile.prototype, "idDefinedJsonFile", undefined);
