/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/compile-node.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import path_1 from "path";
import * as node_command_builder_js_1 from "../builder/node-command-builder";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as aot_compile_mode_enum_js_1 from "../enum/aot-compile-mode-enum";
import * as code_type_enum_js_1 from "../enum/code-type-enum";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as process_utils_js_1 from "../utils/process-utils";
import * as abstract_compile_node_js_1 from "./abstract/abstract-compile-node";
import * as compile_resource_js_1 from "./compile-resource";
import * as generate_loader_json_js_1 from "./generate-loader-json";
export class CompileNode extends abstract_compile_node_js_1.AbstractCompileNode {
  constructor(e, t, o) {
    super(e, t, o), this._log = ohos_logger_js_1.OhosLogger.getLogger(CompileNode.name), this._webpackPath = "./node_modules/webpack/bin/webpack.js", this.webpackConfig = this.codeType === code_type_enum_js_1.CodeType.ETS ? common_const_js_1.CommonConst.ETS_WEBPACK_FILE : common_const_js_1.CommonConst.ACE_RICH_WEBPACK_FILE;
  }
  initTaskDepends() {
    this.declareDependsList(generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name);
  }
  async beforeTask() {
    await super.beforeTask(), this.targetService.getAnBuildMode() !== aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_PARTIAL && fse.removeSync(this.anBuildOutputPath);
  }
  async doTaskAction() {
    const e = this.durationEvent.createSubEvent("generate compile-ArkTs-or-JS-component command", "");
    e.start(), this.validateModuleJson(this._log), await this.doRealLoaderCompile(this.generateLoaderEnv(), e => {
      this.moveReleaseMap(e, this._log);
    }, [this.targetData], e, [0, 1]);
  }
  async doRealLoaderCompile(e, t, o, s, i) {
    e.runtimeOS = this.sdkInfo.isOhos ? common_const_js_1.CommonConst.OPEN_HARMONY : common_const_js_1.CommonConst.HARMONY_OS, e.sdkInfo = `${this.sdkInfo.isOhos}:${this.sdkInfo.getSdkVersion()}:${this.sdkInfo.getEtsComponentVersion()}:${this.sdkInfo.getEtsComponentReleaseType()}`, fse.ensureDirSync(e.cachePath), fse.ensureDirSync(e.aceModuleBuild);
    const r = this.createNodeCommandBuilder();
    this._log._printDebugCommand("NodeEnv", e), this._log._printDebugCommand(`${this.codeType.toUpperCase()}-loader`, r.build());
    const n = this.getInputData(e, r.build());
    s.stop(), s.setLog(s.getName(), hvigor_1.MetricLogType.INFO);
    const a = "submit compile-ArkTs-or-JS-component task to work pool",
      d = this.durationEvent.createSubEvent(a, "");
    d.start(), await new process_utils_js_1.ProcessUtils(n.moduleName, n.taskName).submitExecutionWithoutReturn(this, this.getWorkerPool(), n, t, o, d, i), d.stop(), d.setLog(a, hvigor_1.MetricLogType.INFO);
  }
  createNodeCommandBuilder() {
    const e = /[0-9]+/.exec(process.version),
      t = e ? Number(e[0]) : -1,
      o = new node_command_builder_js_1.NodeCommandBuilder(true).addOpenSslLegacyOption(this.codeType === code_type_enum_js_1.CodeType.JS && 17 <= t).addWebpackPath(this._webpackPath).addWebpackConfig(this.webpackConfig).addBuildMode(this.targetService.isDebug());
    return this.isArkModule && o.addCompilerType("ark"), o;
  }
  getInputData(e, t) {
    const o = {
      cwd: this.codeType === code_type_enum_js_1.CodeType.ETS ? this.sdkInfo.getEtsLoader() : this.sdkInfo.getJsLoader(),
      env: e,
      windowsHide: true
    };
    return {
      moduleName: this.moduleName,
      taskName: this.name,
      commandLine: t,
      commandOptions: o
    };
  }
  generateLoaderEnv() {
    return {
      ...this.commonOption,
      appResource: this.resourceTable,
      rawFileResource: this.rawFileResource,
      aceModuleBuild: this.aceModuleBuild,
      aceModuleRoot: this.sourcePath,
      cachePath: this.getTaskTempDir(this.targetData),
      aceProfilePath: this.resProfileDir,
      aceModuleJsonPath: this.aceModuleJsonPath,
      aceSuperVisualPath: this.aceSuperVisualPath,
      aceBuildJson: path_1.resolve(this.aceBuildJsonDir, build_directory_const_js_1.BuildArtifactConst.LOADER_JSON),
      AnBuildOutPut: path_1.resolve(this.anBuildOutputPath, aot_compile_mode_enum_js_1.AotCompileModeEnum.ARM64_V8A),
      AnBuildMode: this.targetService.getAnBuildMode(),
      apPath: this.targetService.getApAbsolutePath()
    };
  }
  checkLocalDependency(e, t) {
    if (this.codeType !== code_type_enum_js_1.CodeType.JS) return;
    for (const t of e) if (t.getSrcLanguage() !== this.codeType) return;
    const o = this.service.getHarDependencies();
    for (const e of o) e.getDependencyMainFilePath().endsWith(".ets") && t.printErrorExit("JS_MODULE_RELY_ETS_LIB", [this.moduleName, e.getDependencyName(), e.isModuleDependency() ? this.moduleModel.getOhPackageJson5Path() : this.projectModel.getOhPackageJson5Path()], [[this.moduleName]]);
  }
}
