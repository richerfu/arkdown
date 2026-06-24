/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-previewer-compile-resource.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as compile_resources_util_js_1 from "../../utils/compile-resources-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as abstract_compile_resource_js_1 from "./abstract-compile-resource";
export class AbstractPreviewerCompileResource extends abstract_compile_resource_js_1.AbstractCompileResource {
  constructor(e, t) {
    super(e, t), this.compileCommands = [], this.linkCommand = [];
  }
  async compileLink(e, t = wdk_1.noop) {
    for (const t of this.compileCommands) await this.executeCommand(t, e);
    await this.executeCommand(this.linkCommand, e, t);
  }
  async previewCompileLink(e, t = wdk_1.noop) {
    var i;
    const s = "generate compilation link command",
      o = this.durationEvent.createSubEvent(s, "");
    o.start(), this.restoolLinkParamObj = null === (i = this.restoolCommandBuilder) || undefined === i ? undefined : i.restoolLinkParamObj, await this.buildIncrementalCompileLinkFullCommand(), o.stop(), o.setLog(s, hvigor_1.MetricLogType.INFO), await this.compileLink(e, t), await this.previewBuildParamPersistence(), this.moduleModel.isFormWidgetModule(this.targetName) && this.processPreviewFormWidgetModule();
  }
  async previewBuildParamPersistence() {
    const e = this.restoolLinkParamObj.appScopeResources,
      t = this.restoolLinkParamObj.moduleResourcesMap,
      i = this.restoolLinkParamObj.harResourcesMap,
      s = {
        APP_RESOURCES: e || "",
        MODULE_RESOURCES: JSON.stringify((0, hvigor_common_1.strMapToObj)(t)),
        HAR_RESOURCES: JSON.stringify((0, hvigor_common_1.strMapToObj)(i)),
        PREVIEW_GENERATE_SOURCE_DIR: this.projectModel.isFaMode() ? this.getTaskTempDir(this.targetData, build_directory_const_js_1.BuildDirConst.R, false) : this.pathInfo.getPreviewGenerateSourceR(),
        PREVIEW_COMPILE_MODULE_NAMES: compile_resources_util_js_1.CompileResourcesUtil.getRestoolModuleNames(this.service, this.targetName).join(",")
      },
      o = path_1.resolve(this.pathInfo.getModulePreviewPath(), build_directory_const_js_1.BuildArtifactConst.PREVIEW_BUILD_PARAM_JSON);
    await fs_extra_1.outputJSON(o, s);
  }
  async buildIncrementalCompileLinkFullCommand() {
    var e;
    this.linkCommand = this.restoolLinkParamObj.linkCommand, this.linkOutputPath = null === (e = this.restoolLinkParamObj) || undefined === e ? undefined : e.outputPath;
    const t = this.restoolLinkParamObj.appScopeResources,
      i = this.restoolLinkParamObj.moduleResourcesMap,
      s = this.restoolLinkParamObj.harResourcesMap,
      o = this.restoolLinkParamObj.outputPath,
      r = this.restoolLinkParamObj.idsMapPath;
    await fs_extra_1.ensureDir(o);
    const a = (await fs_extra_1.readdir(o)).filter(e => ![build_directory_const_js_1.BuildDirConst.IDS_MAP, build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT].includes(e));
    for (const e of a) await fs_extra_1.remove(path_1.resolve(o, e));
    if (t && t.length > 0) {
      const e = path_1.resolve(this.linkOutputPath, build_directory_const_js_1.BuildDirConst.APP_COMPILED),
        i = [this.getResToolFile(), "-x", t, "-o", e];
      this.compileCommands.push(i), this.linkCommand.push("-i", e), file_util_js_1.FileUtil.checkDirWithoutDelete(e);
    }
    r && file_util_js_1.FileUtil.checkDirWithoutDelete(r), this.linkCommandPushResourcesMap(i), this.linkCommandPushResourcesMap(s), this.linkCommand.push("-o", o), this.sdkInfo.isOhos || this.linkCommand.push("--defined-sysids", path_1.resolve(this.sdkInfo.getHosToolchainsDir(), "id_defined.json"));
  }
  linkCommandPushResourcesMap(e) {
    e.size > 0 && e.forEach((e, t) => {
      const i = path_1.resolve(this.linkOutputPath, e),
        s = [this.getResToolFile(), "-x", t, "-o", i];
      this.compileCommands.push(s), this.linkCommand.push("-i", i), fs_extra_1.existsSync(i) || fs_extra_1.mkdirsSync(i);
    });
  }
}
