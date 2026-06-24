/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/previewer/replace-previewer-page.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as inject_const_js_1 from "../../const/inject-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as file_util_js_1 from "../../utils/file-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as preview_hook_compile_resource_js_1 from "../hook/preview-hook-compile-resource";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as json_util_js_1 from "../../utils/json-util";
import * as copy_preview_profile_js_1 from "./copy-preview-profile";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class ReplacePreviewerPage extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, CommonTask.REPLACE_PREVIEWER_PAGES), this._log = ohos_logger_js_1.OhosLogger.getLogger(ReplacePreviewerPage.name);
  }
  initTaskDepends() {
    this.declareDepends(preview_hook_compile_resource_js_1.PreviewHookCompileResource.name);
    "true" === hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.PREVIEWER_COMPILE_RES_INC) && this.declareDepends(copy_preview_profile_js_1.CopyPreviewProfile.name);
  }
  doTaskAction() {
    this.processPreviewProfile(this._log);
  }
  processPreviewProfile(e) {
    this.appendMainPages();
    let o = true;
    if (hvigor_1.hvigorCore.getExtraConfig().get("compileHspDependencies")) {
      const e = hvigor_1.hvigorCore.getExtraConfig().get("mainModule"),
        s = this.targetData.getModuleModel();
      s.getName() !== e && s.getModuleType() === module_type_enum_js_1.ModuleType.Shared && (o = false);
    }
    o && this.replacePreviewPage(e);
  }
  replacePreviewPage(e) {
    const o = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.PREVIEWER_REPLACE_PAGE);
    return !!o && (this.replacePagesInPreview(e, [o]), true);
  }
  appendMainPages() {
    if (this.moduleModel.getModuleType() !== module_type_enum_js_1.ModuleType.Shared) return;
    const e = path_1.resolve(this.pathInfo.getIntermediatesRes(), common_const_js_1.CommonConst.MODULE_JSON),
      o = json_util_js_1.JsonUtil.getJson5Obj(e);
    Object.assign(o.module, {
      pages: "$profile:main_pages"
    });
    const s = this.pathInfo.getPreviewIntermediatesResModuleJsonPath();
    file_util_js_1.FileUtil.checkFile(s), fs_extra_1.writeJSONSync(s, o);
  }
}
