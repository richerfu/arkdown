/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-package-har.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_2 from "@ohos/hvigor";
import * as fs from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as log_combine_type_js_1 from "../../utils/log/log-combine-type";
import * as npm_utils_js_1 from "../../utils/npm-utils";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as ohos_har_task_js_1 from "../task/ohos-har-task";
import * as npm_command_builder_js_1 from "../../builder/npm-command-builder";
export class AbstractPackageHar extends ohos_har_task_js_1.OhosHarTask {
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntries([this.taskTmpDir, this.outputDir], {
      isDirectory: true
    });
  }
  constructor(t, e) {
    super(t, e), this.outputDir = this.pathInfo.getModuleBuildOutputPath(), this.npmPath = path_1.dirname(process.execPath), this.npmCommand = new npm_command_builder_js_1.NpmCommandBuilder(this.npmPath).addAllParams(["pack"]);
  }
  declareExecutionCommand() {
    return `${this.npmCommand.toString()}`;
  }
  async doTaskAction() {
    fs.emptyDirSync(this.outputDir), this.isOhpmProject ? await this.executeOhpmPack() : await this.executeNpmPack(), this.moveSourceMap();
  }
  async executeOhpmPack() {
    var t;
    const e = null === (t = this.targetData.getTargetOpt().output) || undefined === t ? undefined : t.artifactName,
      i = e ? `${e}` : `${this.moduleName}`;
    fs.emptyDirSync(this.outputDir), await (0, npm_utils_js_1.execOhpmPack)(this.taskTmpDir, path_1.resolve(this.outputDir, `${i}.har`), this.durationEvent);
  }
  async executeNpmPack() {
    const t = {
        moduleName: this.moduleName,
        taskName: this.name,
        commandLine: this.npmCommand,
        commandOptions: {
          cwd: this.taskTmpDir
        }
      },
      e = t => {
        fs.copySync(this.taskTmpDir, this.outputDir, {
          filter: e => !fs.lstatSync(e).isDirectory() && e.endsWith(t) || e === this.taskTmpDir
        });
      },
      i = "execute npm packaging command",
      r = this.durationEvent.createSubEvent(i, "");
    if (r.start(), !new process_utils_js_1.ProcessUtils(this.moduleName, this.name).submitSyncExecutionWithReturn(this, this.getWorkerPool(), "packageHar", t, e)) {
      e((await new process_utils_js_1.ProcessUtils(this.moduleName, this.name).execute(t.commandLine, t.commandOptions, log_combine_type_js_1.LogCombineType.DEBUG)).stdout.toString());
    }
    r.stop(), r.setLog(i, hvigor_1.MetricLogType.INFO);
  }
  moveSourceMap() {
    if (!this.sdkInfo.hasRollUpPluginInEtsLoader) return;
    const t = this.pathInfo.getSourceMapOriginPath(this.targetService);
    if (fs.existsSync(t)) if (hvigor_1.coreParameter.properties[hvigor_2.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]) {
      const e = hvigor_1.coreParameter.properties[hvigor_2.OHOS_ARK_COMPILE_SOURCE_MAP_DIR],
        i = path_1.isAbsolute(e) ? path_1.resolve(e, this.moduleName) : path_1.resolve(this.projectModel.getProjectDir(), "hvigor", e, this.moduleName);
      fs.ensureDirSync(i), fs.copyFileSync(t, path_1.resolve(i, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP));
    } else fs.ensureDirSync(this.pathInfo.getDefaultSourceMapPath()), fs.copyFileSync(t, path_1.resolve(this.pathInfo.getDefaultSourceMapPath(), build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP));
  }
}
