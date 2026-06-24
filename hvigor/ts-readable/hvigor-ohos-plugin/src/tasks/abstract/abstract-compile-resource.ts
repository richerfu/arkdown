/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-compile-resource.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as log4js_1 from "log4js";
import * as ohos_trace_js_1 from "../../common/trace/ohos-trace";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as inject_const_js_1 from "../../const/inject-const";
import * as compile_resources_util_js_1 from "../../utils/compile-resources-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as abstract_resource_task_js_1 from "./abstract-resource-task";
import * as json_util_js_1 from "../../utils/json-util";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as form_util_js_1 from "../../utils/form-util";
export class AbstractCompileResource extends abstract_resource_task_js_1.AbstractResource {
  constructor(e, t) {
    super(e, t), this.intermediatesResDir = this.pathInfo.getIntermediatesRes(), this.isPreview = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.BUILD_ROOT) === build_directory_const_js_1.BuildDirConst.PREVIEW_BUILD_PATH;
  }
  getEnabled() {
    const e = this.node.getTaskContainer().findTask(`${this.targetName}@${this.processProfileTask}`);
    if (undefined === e) throw new Error(`Resource compile task initialize failed, task ${this.processProfileTask} not found.`);
    return this.restoolCommandBuilder = e.restoolCommandBuilder, this.restoolConfigBuilder = e.restoolConfigBuilder, super.getEnabled();
  }
  getCommand() {
    return [this.getResToolFile(), "-l", this.resConfigFilePath];
  }
  beforeTask() {
    file_util_js_1.FileUtil.checkDirWithoutDelete(this.intermediatesResDir), file_util_js_1.FileUtil.checkDirWithoutDelete(this.generateSourceRDir);
  }
  declareInputs() {
    const e = new Map();
    return this.isPreview && (e.set("PREVIEWER_REPLACE_PAGE", JSON.stringify(hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.PREVIEWER_REPLACE_PAGE))), e.set("PREVIEWER_REPLACE_SRCPATH", JSON.stringify(hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.PREVIEWER_REPLACE_SRC_PATH)))), e.set("TARGET_CONFIG", JSON.stringify(this.targetData.getTargetOpt())), e;
  }
  declareExecutionTool() {
    return this.getResToolFile();
  }
  declareInputFiles() {
    return this.restoolConfigBuilder.inputFiles;
  }
  declareOutputFiles() {
    return this.restoolConfigBuilder.outputFiles.addEntries([this.intermediatesResDir, this.generateSourceRDir], {
      isDirectory: true
    });
  }
  getResToolFile() {
    return this.sdkInfo.getRestool();
  }
  async compilationProcess(e, t) {
    this.precheck(e) && (await this.invokeRestool(t));
  }
  precheck(e) {
    var t;
    return e === compile_resources_util_js_1.CompileResourceBuildCommandType.FILE ? this.restoolConfigBuilder.existsResourceDir : !!(null === (t = this.restoolCommandBuilder) || undefined === t ? undefined : t.existsResourceDir);
  }
  async invokeRestool(e) {}
  async executeCommand(e, t, i = wdk_1.noop, o) {
    var s;
    let r;
    r = this.projectModel.isFaMode() ? null === (s = this.moduleModel.getJsonObjByTargetName(this.targetData.getTargetName())) || undefined === s ? undefined : s.app.bundleName : this.projectModel.getDefaultBundleName(), t._printDebugCommand(this.getResToolFile(), (0, hvigor_1.replaceBundleName)(e, r));
    const l = "execute compile resource command using restool",
      n = this.durationEvent.createSubEvent(l, "");
    n.start(), await new process_utils_js_1.ProcessUtils(this.moduleName, this.name).execute(e, o, undefined, (e, i) => (this.restoolReportHandler(e), this.iconSizeWarningHandler(e, i, t))), n.stop(), n.setLog(l, hvigor_1.MetricLogType.INFO), await i();
  }
  iconSizeWarningHandler(e, t, i) {
    var o;
    if (!((null === (o = e.stderr) || undefined === o ? undefined : o.length) > 0)) return false;
    const s = e.stderr;
    return !(!/Warning: the png width and height not equal/.test(s) && !/Warning: The width or height of the .+ file referenced by the icon exceeds the limit+./.test(s)) && (JSON.stringify(i.getLevel()) === JSON.stringify(log4js_1.levels.DEBUG) && i.warn(s), true);
  }
  restoolReportHandler(e) {
    var t, i, o, s;
    const r = (e.stdout + e.stderr).split("\n"),
      l = r.findIndex(e => e.startsWith("Processing report:"));
    if (l >= 0 && l + 3 < r.length) {
      const e = /\d+\\.\d+|\d+/g,
        n = null === (i = null === (t = r[l + 2]) || undefined === t ? undefined : t.match(e)) || undefined === i ? undefined : i.map(e => parseFloat(e)),
        a = null === (s = null === (o = r[l + 3]) || undefined === o ? undefined : o.match(e)) || undefined === s ? undefined : s.map(e => parseFloat(e));
      if (!Array.isArray(n) || !Array.isArray(a) || 0 === n[0] || 0 === a[2]) return;
      const [d, u] = n,
        [c, _, m, g] = a;
      ohos_trace_js_1.ohosTrace.traceRestoolCompression(this.moduleModel.getName(), {
        TIMESTAMP: Date.now(),
        TRANSCODE_FILES: d,
        TRANSCODE_SUCCESS_RATE: c / d,
        TRANSCODE_TOTAL_TIME: u,
        TRANSCODE_EXPANSION_RATE: g / m
      });
    }
  }
  async retainDeviceType(e, t, i) {
    var o, s, r, l;
    const n = e.findRelatedTargetData(t);
    if ((0, module_task_service_js_1.checkHasTargetApplyProduct)(this.targetData.getModuleModel(), this.targetData.getTargetName(), this.targetData.getProduct().name) && n) {
      const a = n.getTargetDeviceType(),
        d = e.getTargetDeviceType().filter(e => (null == a ? undefined : a.indexOf(e)) > -1),
        u = path_1.resolve(e.getPathInfo().getIntermediatesRes(), t, common_const_js_1.CommonConst.CONFIG_JSON),
        c = json_util_js_1.JsonUtil.getJson5Obj(u);
      if (c.module.deviceType = d, this.isFaMode) {
        const e = this.service.getProjectModel(),
          i = e.getModuleModelByName(t),
          a = path_1.resolve(null !== (o = null == i ? undefined : i.getProjectDir()) && undefined !== o ? o : path_1.resolve(e.getProjectDir(), t), `src/main/${common_const_js_1.CommonConst.CONFIG_JSON}`),
          d = res_model_loader_js_1.resModelLoader.getConfigJson(a),
          u = null !== (r = null === (s = n.getTargetOpt().config) || undefined === s ? undefined : s.distroFilter) && undefined !== r ? r : null === (l = null == d ? undefined : d.module) || undefined === l ? undefined : l.distroFilter;
        u && (c.module.distroFilter = u);
      }
      await fs_extra_1.outputJson(u, c, {
        spaces: "\t"
      }), 0 === d.length && i.warn(`The device types of ${this.moduleModel.getName()} and ${t} are different. Invalid HAP ${this.moduleModel.getName()}.`);
    }
  }
  replaceTargetPages(e, t) {
    var i;
    const o = null === (i = e.getTargetOpt().source) || undefined === i ? undefined : i.pages;
    undefined !== o && this.replacePages(t, o);
  }
  processFormExtensionAbilities() {
    this.moduleModel.isFormExtensionModule(this.targetName) ? this.recoverFormExtensionModule() : this.moduleModel.isFormWidgetModule(this.targetName) && this.processFormWidgetModule();
  }
  recoverFormExtensionModule() {
    const e = this.targetService.getTargetData().getModuleSourceSetModel().getModuleTargetRes().getModuleJsonOpt();
    if (!e.module.extensionAbilities) return;
    const t = path_1.resolve(this.intermediatesResDir, common_const_js_1.CommonConst.MODULE_JSON),
      i = json_util_js_1.JsonUtil.getJson5Obj(t);
    e.module.extensionAbilities.filter(e => "form" === e.type).forEach(e => {
      i.module.extensionAbilities || (i.module.extensionAbilities = []), i.module.extensionAbilities.push(e);
    }), fs_extra_1.writeJSONSync(t, i);
  }
  processFormWidgetModule() {
    const e = path_1.resolve(this.intermediatesResDir, common_const_js_1.CommonConst.ARK_MODULE_JSON);
    this.writeFormExtensionAbilities(e);
  }
  writeFormExtensionAbilities(e) {
    var t;
    const i = json_util_js_1.JsonUtil.getJson5Obj(e),
      o = form_util_js_1.FormUtil.getFormExtensionModuleByTargetName(this.moduleModel, this.targetName).getJsonObjByTargetName(this.targetName);
    if (!(null === (t = null == o ? undefined : o.module) || undefined === t ? undefined : t.extensionAbilities)) return;
    const s = o.module.extensionAbilities.filter(e => "form" === e.type);
    s.length && !i.module.extensionAbilities && (i.module.extensionAbilities = []), s.forEach(e => {
      i.module.extensionAbilities || (i.module.extensionAbilities = []);
      const t = {
        name: e.name,
        type: e.type,
        metadata: e.metadata
      };
      i.module.extensionAbilities.push(t);
    }), fs_extra_1.writeJSONSync(e, i);
  }
  processPreviewFormWidgetModule() {
    const e = path_1.resolve(this.intermediatesResDir, common_const_js_1.CommonConst.MODULE_JSON);
    this.writeFormExtensionAbilities(e);
  }
}
