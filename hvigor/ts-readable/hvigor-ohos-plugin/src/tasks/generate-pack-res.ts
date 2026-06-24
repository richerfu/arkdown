/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-pack-res.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as packing_tool_options_js_1 from "../builder/inner-java-command-builder/packing-tool-options";
import * as process_utils_js_1 from "../utils/process-utils";
import * as task_names_js_1 from "./common/task-names";
import * as make_project_pack_info_js_1 from "./make-project-pack-info";
import * as base_pack_app_task_js_1 from "./base/base-pack-app-task";
import * as duplicate_dependency_check_js_1 from "./duplicate-dependency-check";
import * as process_project_privacy_profile_js_1 from "./pac-process/process-project-privacy-profile";
var Task = task_names_js_1.TaskNames.Task;
export class GeneratePackRes extends base_pack_app_task_js_1.BasePackAppTask {
  declareExecutionCommand() {
    var e;
    return `${null === (e = this.packResCommand) || undefined === e ? undefined : e.toString()}`;
  }
  declareInputs() {
    return new Map().set("needPackRes", this.needPackRes);
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet().addEntry(this.packInfoPath, {
      isDirectory: false
    });
    return this.needPackRes && e.addEntry(this.entryCardDir, {
      isDirectory: true
    }), e;
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    return this.needPackRes && e.addEntry(this.packResPath, {
      isDirectory: false
    }), e;
  }
  constructor(e, s) {
    super(e, s, Task.GeneratePackRes), this.needPackRes && (this.packResCommand = new packing_tool_options_js_1.PackingToolOptions().addCalledJarFile(this.packageTool).addMode("res").addEntryCardPath(this.entryCardDir).addPackInfoPath(this.packInfoPath).addOutPath(this.packResPath).force(true).build());
  }
  initTaskDepends() {
    this.dependsOn(duplicate_dependency_check_js_1.DuplicateDependencyCheck.name), this.dependsOn(make_project_pack_info_js_1.MakeProjectPackInfo.name), this.dependsOn(process_project_privacy_profile_js_1.ProcessProjectPrivacyProfile.name);
  }
  async doTaskAction() {
    await this.generatePackRes();
  }
  async generatePackRes() {
    this.needPackRes ? (this._log._printDebugCommand("PackRes", this.packResCommand), await new process_utils_js_1.ProcessUtils().execute(this.packResCommand)) : fs_extra_1.rmSync(this.packResPath, {
      force: true
    });
  }
}
