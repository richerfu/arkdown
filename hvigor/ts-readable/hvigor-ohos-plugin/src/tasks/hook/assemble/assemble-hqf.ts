/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/assemble/assemble-hqf.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as task_names_js_1 from "../../common/task-names";
import * as prepare_quickfix_js_1 from "../../prepare-quickfix";
import * as abstract_module_hook_task_js_1 from "../abstract-module-hook-task";
export class AssembleHqf extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(e) {
    super(e, false, task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_HQF), this.registryAction = () => async () => {
      const e = path_1.resolve(hvigor_1.PathUtil.getHvigorCacheDir(), "outputs", "patch", "patch.json");
      let s = await prepare_quickfix_js_1.PrepareQuickfix.getCurrentHqfVersion();
      AssembleHqf.isAdd && undefined === hvigor_1.hvigorCore.getExtraConfig().get("hotReload-module") && (s += 1, AssembleHqf.isAdd = false, await fs_extra_1.ensureDir(path_1.dirname(e)), await fs_extra_1.writeJSON(e, {
        patchVersionCode: s
      }));
    };
  }
  initTaskDepends(e) {
    throw new Error("Method not implemented.");
  }
}
AssembleHqf.isAdd = true, AssembleHqf.versionCode = undefined;
