/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-resource-task.js
 */
import * as restool_command_builder_js_1 from "../../builder/restool-command-builder";
import * as restool_file_config_builder_js_1 from "../../builder/restool-file-config-builder";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as compile_resources_util_js_1 from "../../utils/compile-resources-util";
import * as har_target_util_js_1 from "../../utils/har-target-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
const _log = ohos_logger_js_1.OhosLogger.getLogger("Resource");
export class AbstractResource extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e, o) {
    super(e, o), this.outputDir = this.pathInfo.getIntermediatesRes(), this.restoolConfigBuilder = this.getRestoolConfigBuilder();
  }
  getRestoolConfigBuilder() {
    const e = this.initConfigTargetResource(),
      o = compile_resources_util_js_1.CompileResourcesUtil.getRestoolModuleNames(this.service, this.targetName).join(",");
    _log.debug(`restool module names: ${o}; moduleName=${this.moduleName}, taskName=${this.name}`), e.addModules(o).forceDelete();
    const t = new Map();
    return har_target_util_js_1.HarTargetUtil.getHarPathToResourcesDirMap(t, this.targetService), t.set(this.pathInfo.getModulePath(), [this.pathInfo.getResourceStr()]), t.forEach(o => {
      o.forEach(o => {
        e.addInputDir(o, build_directory_const_js_1.ResToolInputTypeConst.HAR);
      });
    }), e;
  }
  initConfigTargetResource() {
    const e = new restool_file_config_builder_js_1.RestoolConfigBuilder();
    return this.targetService.getResourceDirs().forEach(o => e.addInputDir(o, "Module")), e;
  }
  getRestoolCommandBuilder() {
    const e = this.initCommandTargetResource();
    return e.addModules(compile_resources_util_js_1.CompileResourcesUtil.getRestoolModuleNames(this.service, this.targetName).join(",")).forceDelete(), e;
  }
  initCommandTargetResource() {
    const e = new restool_command_builder_js_1.RestoolCommandBuilder(this.sdkInfo.getRestool()),
      o = new Map(),
      t = new Map();
    this.targetService.getResourceDirs().forEach((e, t) => {
      const s = compile_resources_util_js_1.CompileResourcesUtil.getCompileTargetFolderBySeq(build_directory_const_js_1.BuildDirConst.MODULE_COMPILED, t);
      o.set(e, s);
    });
    const s = new Map(),
      r = [];
    return har_target_util_js_1.HarTargetUtil.getHarPathToResourcesDirMap(s, this.targetService), s.forEach(e => {
      e.forEach(e => {
        r.push(e);
      });
    }), r.forEach((e, o) => {
      const s = compile_resources_util_js_1.CompileResourcesUtil.getCompileTargetFolderBySeq(build_directory_const_js_1.BuildDirConst.HAR_COMPILED, o);
      t.set(e, s);
    }), e.addModuleResourcesMap(o), e.addHarResourcesMap(t), e;
  }
}
