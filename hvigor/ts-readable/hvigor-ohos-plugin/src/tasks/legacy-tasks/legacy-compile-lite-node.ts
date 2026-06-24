/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-compile-lite-node.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as node_command_builder_js_1 from "../../builder/node-command-builder";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as file_util_js_1 from "../../utils/file-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as task_names_js_1 from "../common/task-names";
import * as compile_node_js_1 from "../compile-node";
import * as legacy_hook_compile_resource_js_1 from "./hook/legacy-hook-compile-resource";
import * as legacy_generate_js_manifest_js_1 from "./legacy-generate-js-manifest";
import * as legacy_generate_loader_json_js_1 from "./legacy-generate-loader-json";
export class LegacyCompileLiteNode extends compile_node_js_1.CompileNode {
  constructor(e) {
    super(e, code_type_enum_js_1.CodeType.JS, task_names_js_1.TaskNames.LegacyFATask.COMPILE_LITE_NODE), this._logger = ohos_logger_js_1.OhosLogger.getLogger(LegacyCompileLiteNode.name);
  }
  declareOutputFiles() {
    const e = path_1.resolve(this.pathInfo.getInterMediatesLoaderOutLitePath(), this.codeType);
    return new hvigor_1.FileSet().addEntry(e, {
      isDirectory: true
    });
  }
  taskShouldDo() {
    return this.service.hasLiteDevice();
  }
  initTaskDepends() {
    this.declareDepends(legacy_generate_js_manifest_js_1.LegacyGenerateJsManifest.name), this.declareDepends(legacy_hook_compile_resource_js_1.LegacyHookCompileResource.name), this.declareDepends(legacy_generate_loader_json_js_1.LegacyGenerateLoaderJson.name);
  }
  async doTaskAction() {
    const e = this.service.getModuleModel(),
      t = e.getSourceSetByTargetName(this.targetName).getCodeMap().get(this.codeType);
    if (undefined === t) return void this._logger.printErrorExit("LITE_WEARABLE_ONLY_SUPPORT_JS", [this.pathInfo]);
    const o = this.durationEvent.createSubEvent("generate compile-JS-component command", "");
    o.start();
    const s = t.getSrcPath(),
      a = this.pathInfo,
      i = path_1.resolve(a.getIntermediatesLegacyManifestJson()),
      r = path_1.resolve(this.pathInfo.getInterMediatesLoaderOutLitePath(), code_type_enum_js_1.CodeType.JS),
      _ = e.getLegacyAbilities(this.targetName);
    this.checkLocalDependency(_, this._logger), file_util_js_1.FileUtil.checkDirWithoutDelete(a.getIntermediatesLiteBinSource()), fs_extra_1.copySync(e.getSourceSetByTargetName().getLegacyModuleTargetRes().getResourcePath(), a.getIntermediatesLiteBinSource(), {
      filter: e => fs_extra_1.statSync(e).isDirectory() || e.toLowerCase().endsWith(".png") || e.toLowerCase().endsWith(".jpg"),
      recursive: true
    });
    for (let e = 0; e < _.length; e++) {
      const t = _[e],
        n = {
          ...this.commonOption,
          abilityType: t.getType(),
          aceManifestPath: path_1.resolve(i, t.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LEGACY_MANIFEST_JSON),
          aceModuleRoot: path_1.resolve(s, t.getRelateSrcPath()),
          aceModuleBuild: path_1.resolve(r, t.getRelateSrcPath()),
          hapMode: (!this.targetService.isDebug()).toString(),
          img2bin: "true",
          iconPath: a.getIntermediatesLiteBinSource(),
          cachePath: this.getTaskTempDir(this.targetData),
          aceBuildJson: path_1.resolve(a.getIntermediatesLoaderPath(), t.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LOADER_JSON)
        };
      await this.doRealLoaderCompile(n, (e, t) => {
        this.moveReleaseMap(e, this._logger, t);
      }, [this.targetData, `js/${t.getRelateSrcPath()}`], o, [0, 1]);
    }
  }
  moveReleaseMap(e, t, o = this.codeType) {
    const s = path_1.resolve(e.getPathInfo().getInterMediatesLoaderOutLitePath(), o, build_directory_const_js_1.BuildArtifactConst.RELEASE_MAP);
    if (!fs_extra_1.existsSync(s)) return;
    const a = path_1.resolve(this.getTaskTempDir(e), o, build_directory_const_js_1.BuildArtifactConst.RELEASE_MAP);
    fs_extra_1.existsSync(a) && fs_extra_1.removeSync(a), fs_extra_1.moveSync(s, a), t.debug(`move ${s} to ${a}`);
  }
  async doRealLoaderCompile(e, t, o, s, a) {
    const i = new node_command_builder_js_1.NodeCommandBuilder(true).addWebpackPath("./node_modules/webpack/bin/webpack.js").addWebpackConfig(common_const_js_1.CommonConst.ACE_LITE_WEBPACK_FILE).addBuildMode(this.targetService.isDebug());
    this._logger._printDebugCommand("NodeEnv", e), this._logger._printDebugCommand("lite-loader", i.build());
    const r = this.getInputData(e, i.build());
    s.stop(), s.setLog(s.getName(), hvigor_1.MetricLogType.INFO);
    const _ = "submit compile-JS-component task to work pool",
      n = this.durationEvent.createSubEvent(_, "");
    n.start(), await new process_utils_js_1.ProcessUtils(r.moduleName, r.taskName).submitExecutionWithoutReturn(this, this.getWorkerPool(), r, t, o, s, a), n.stop(), n.setLog(_, hvigor_1.MetricLogType.INFO);
  }
}
