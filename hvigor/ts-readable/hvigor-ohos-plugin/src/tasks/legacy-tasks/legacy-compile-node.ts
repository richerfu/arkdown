/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-compile-node.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as ability_type_enum_js_1 from "../../enum/ability-type-enum";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as compile_node_js_1 from "../compile-node";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as legacy_hook_compile_resource_js_1 from "./hook/legacy-hook-compile-resource";
import * as legacy_generate_js_manifest_js_1 from "./legacy-generate-js-manifest";
import * as legacy_generate_loader_json_js_1 from "./legacy-generate-loader-json";
export class LegacyCompileNode extends compile_node_js_1.CompileNode {
  constructor(e, t, s = task_names_js_1.TaskNames.LegacyFATask.COMPILE_NODE) {
    super(e, t, s), this.log = ohos_logger_js_1.OhosLogger.getLogger(LegacyCompileNode.name), this.manifestDir = this.pathInfo.getIntermediatesLegacyManifestJson();
    const a = this.service.getRelatedEntryModules(),
      i = this.service.getModuleModel().getModuleType(),
      r = module_task_service_js_1.ModuleTaskService.checkEntryModules(i, a) ? a[0] : "";
    this.resourceTablePath = path_1.resolve(this.pathInfo.getIntermediatesRes(), r, build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT), this.rawFileResource = path_1.resolve(this.pathInfo.getIntermediatesRes(), r, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES, build_directory_const_js_1.BuildDirConst.RAW_FILE);
  }
  declareOutputFiles() {
    return new hvigor_1.FileSet().addEntry(this.aceModuleBuild, {
      isDirectory: true
    });
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet().addEntry(this.manifestDir, {
      isDirectory: true
    }).addEntry(this.aceBuildJsonDir, {
      isDirectory: true
    }).addEntries(this.getAllExistDependentMainPath()).addEntries(this.getAllExistHarSrcPath(), {
      isDirectory: true
    });
    return this.targetService.getResourceDirs().length > 0 && e.addEntry(this.resourceTablePath), fs_extra_1.existsSync(this.rawFileResource) && e.addEntry(this.rawFileResource, {
      isDirectory: true
    }), this.sourcePath && fs_extra_1.existsSync(this.sourcePath) && e.addEntry(this.sourcePath, {
      isDirectory: true
    }), this.aceSuperVisualPath && fs_extra_1.existsSync(this.aceSuperVisualPath) && e.addEntry(this.aceSuperVisualPath, {
      isDirectory: true
    }), e;
  }
  initTaskDepends() {
    this.declareDepends(legacy_generate_js_manifest_js_1.LegacyGenerateJsManifest.name), this.declareDepends(legacy_hook_compile_resource_js_1.LegacyHookCompileResource.name), this.declareDepends(legacy_generate_loader_json_js_1.LegacyGenerateLoaderJson.name);
  }
  taskShouldDo() {
    return this.targetData.getTargetDeviceTypeClasses().includes(device_type_const_js_1.DeviceTypeConst.RICH) && super.taskShouldDo();
  }
  async doTaskAction() {
    const e = this.durationEvent.createSubEvent("generate compile-ArkTs-or-JS-component command", "");
    e.start();
    const t = this.moduleModel.getLegacyAbilities(this.targetData.getTargetName());
    this.checkLocalDependency(t, this.log), this.validateAbility(this.log);
    for (const s of t) {
      if (s.getSrcLanguage() !== this.codeType) continue;
      const t = {
        ...this.commonOption,
        abilityType: s.getType(),
        aceManifestPath: path_1.resolve(this.manifestDir, s.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LEGACY_MANIFEST_JSON),
        appResource: this.resourceTablePath,
        rawFileResource: this.rawFileResource,
        aceModuleRoot: path_1.resolve(this.sourcePath, s.getRelateSrcPath()),
        aceModuleBuild: path_1.resolve(this.aceModuleBuild, s.getRelateSrcPath()),
        cachePath: path_1.resolve(this.getTaskTempDir(this.targetData), s.getRelateSrcPath()),
        aceSuperVisualPath: path_1.resolve(this.aceSuperVisualPath, s.getRelateSrcPath()),
        aceBuildJson: path_1.resolve(this.aceBuildJsonDir, s.getRelateSrcPath(), build_directory_const_js_1.BuildArtifactConst.LOADER_JSON)
      };
      await this.doRealLoaderCompile(t, (e, t) => {
        this.moveReleaseMap(e, this.log, t);
      }, [this.targetData, `js/${s.getRelateSrcPath()}`], e, [0, 1]);
    }
    await this.compileTestRunner();
  }
  async compileTestRunner() {
    const e = this.needCompileTestRunner();
    if (!e[0]) return;
    const t = this.durationEvent.createSubEvent("generate compile-ArkTs-or-JS-component command for test runner", "");
    t.start();
    const s = e[1],
      a = {
        ...this.commonOption,
        abilityType: ability_type_enum_js_1.AbilityTypeEnum.TEST_RUNNER,
        appResource: this.resourceTablePath,
        rawFileResource: this.rawFileResource,
        aceModuleRoot: path_1.resolve(this.sourcePath, s),
        aceModuleBuild: path_1.resolve(this.aceModuleBuild, s),
        cachePath: path_1.resolve(this.getTaskTempDir(this.targetData), s)
      };
    await this.doRealLoaderCompile(a, wdk_1.noop, [], t, [0, 1]);
  }
  needCompileTestRunner() {
    var e, t, s;
    const a = this.targetData.getTargetName(),
      i = null === (s = null === (t = null === (e = this.moduleModel.getSourceSetByTargetName(a).getLegacyModuleTargetRes().getConfigJsonOpt()) || undefined === e ? undefined : e.module) || undefined === t ? undefined : t.testRunner) || undefined === s ? undefined : s.srcPath;
    return ["ohosTest" === a && undefined !== i && this.codeType === code_type_enum_js_1.CodeType.JS, i];
  }
}
