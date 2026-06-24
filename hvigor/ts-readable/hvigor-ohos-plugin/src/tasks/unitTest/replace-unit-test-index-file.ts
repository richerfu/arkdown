/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/replace-unit-test-index-file.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as inject_const_js_1 from "../../const/inject-const";
import * as json_util_js_1 from "../../utils/json-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class ReplaceUnitTestIndexFile extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, CommonTask.REPLACE_UNIT_TEST_INDEX_FILE), this._log = ohos_logger_js_1.OhosLogger.getLogger(ReplaceUnitTestIndexFile.name), this.PAGE_JSON = "main_pages", this.PAGE_JSON_FILE = `${this.PAGE_JSON}.json`, this.PAGE_JSON_RESOURCE = `$profile:${this.PAGE_JSON}`;
  }
  initTaskDepends() {
    this.declareDepends(compile_resource_js_1.CompileResource.name);
  }
  doTaskAction() {
    var e;
    const s = path_1.resolve(this.pathInfo.getIntermediatesRes(), common_const_js_1.CommonConst.MODULE_JSON),
      t = path_1.resolve(this.pathInfo.getIntermediatesResProfilePath(), this.PAGE_JSON_FILE);
    if (this.moduleModel.isHarModule()) return this.writeModuleJson(s), void this.writeMainPages(t);
    const o = json_util_js_1.JsonUtil.getJson5Obj(s),
      i = null === (e = null == o ? undefined : o.module) || undefined === e ? undefined : e.pages;
    if (this.moduleModel.isHspModule()) return i || this.writeModuleJson(s), void this.writeMainPages(t);
    this.moduleModel.isHapModule() && !i && (this.writeModuleJson(s), this.writeMainPages(t));
    const _ = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.UNIT_TEST_REPLACE_PAGE);
    _ && this.replacePagesInPreview(this._log, [_]);
  }
  writeModuleJson(e) {
    let s;
    if (fs_extra_1.existsSync(e)) s = fs_extra_1.readJsonSync(e), (null == s ? undefined : s.module) && (s.module.pages = this.PAGE_JSON_RESOURCE);else {
      const e = this.targetData.getPathInfo().getIntermediatesProcessProfile();
      s = json_util_js_1.JsonUtil.getJson5Obj(e), (null == s ? undefined : s.module) ? s.module.pages = this.PAGE_JSON_RESOURCE : s = {
        module: {
          pages: this.PAGE_JSON_RESOURCE
        }
      };
    }
    fs_extra_1.ensureFileSync(e), fs_extra_1.writeJSONSync(e, s);
  }
  writeMainPages(e) {
    const s = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.UNIT_TEST_REPLACE_PAGE);
    if (!s) return;
    const t = {
      src: [s]
    };
    fs_extra_1.ensureFileSync(e), fs_extra_1.writeJSONSync(e, t);
  }
}
