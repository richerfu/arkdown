/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/package-shared-har.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as fs from "fs-extra";
import path_1 from "path";
import * as npm_command_builder_js_1 from "../../builder/npm-command-builder";
import * as log_combine_type_js_1 from "../../utils/log/log-combine-type";
import * as npm_utils_js_1 from "../../utils/npm-utils";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
var Task = task_names_js_1.TaskNames.Task;
export class PackageSharedHar extends ohos_hap_task_js_1.OhosHapTask {
  constructor(t) {
    super(t, Task.PACKAGE_SHARED_HAR), this.taskTmpDir = this.getTaskTempDir(this.targetData, "shared_har", false), this.npmPath = path_1.dirname((0, npm_utils_js_1.getNpmPath)()), this.npmCommand = new npm_command_builder_js_1.NpmCommandBuilder(this.npmPath).addAllParams(["pack"]), this.outputDir = this.pathInfo.getModuleBuildOutputPath();
  }
  declareExecutionCommand() {
    return `${this.npmCommand.toString()}`;
  }
  declareExecutionEnv() {
    const t = new Map();
    return fs.existsSync(this.taskTmpDir) && t.set("cwd", this.taskTmpDir), t;
  }
  declareInputs() {
    return super.declareInputs().set("isOhpmProject", this.isOhpmProject);
  }
  declareInputFiles() {
    const t = super.declareInputFiles();
    return fs.existsSync(this.taskTmpDir) && t.addEntry(this.taskTmpDir, {
      isDirectory: true
    }), t;
  }
  declareOutputFiles() {
    return super.declareOutputFiles().addEntry(this.outputDir, {
      isDirectory: true
    });
  }
  initTaskDepends() {
    this.declareDepends(Task.PREPARE_SHARED_HAR_RES.name);
  }
  async doTaskAction() {
    this.isOhpmProject ? await this.executeOhpmPack() : await this.executeNpmPack();
  }
  async executeNpmPack() {
    const t = "generate npm packaging command",
      e = this.durationEvent.createSubEvent(t, "");
    e.start();
    const s = {
        moduleName: this.moduleName,
        taskName: this.name,
        commandLine: this.npmCommand,
        commandOptions: {
          cwd: this.taskTmpDir
        }
      },
      a = t => {
        fs.copySync(this.taskTmpDir, this.outputDir, {
          filter: e => !fs.lstatSync(e).isDirectory() && e.endsWith(t) || e === this.taskTmpDir
        });
      };
    e.stop(), e.setLog(t, hvigor_1.MetricLogType.INFO);
    const i = "submit npm packaging task to work pool",
      r = this.durationEvent.createSubEvent(i, "");
    if (r.start(), !new process_utils_js_1.ProcessUtils(this.moduleName, this.name).submitSyncExecutionWithReturn(this, this.getWorkerPool(), "packageHar", s, a)) {
      const t = "execute npm packaging command",
        e = r.createSubEvent(t, "");
      e.start();
      const i = await new process_utils_js_1.ProcessUtils(this.moduleName, this.name).execute(s.commandLine, s.commandOptions, log_combine_type_js_1.LogCombineType.DEBUG);
      e.stop(), e.setLog(t, hvigor_1.MetricLogType.INFO), a(i.stdout.toString());
    }
    r.stop(), r.setLog(i, hvigor_1.MetricLogType.INFO);
  }
  async executeOhpmPack() {
    var t;
    const e = null === (t = this.targetData.getTargetOpt().output) || undefined === t ? undefined : t.artifactName,
      s = e ? `${e}` : `${this.moduleName}`;
    await (0, npm_utils_js_1.execOhpmPack)(this.taskTmpDir, path_1.resolve(this.outputDir, `${s}.har`), this.durationEvent);
  }
}
