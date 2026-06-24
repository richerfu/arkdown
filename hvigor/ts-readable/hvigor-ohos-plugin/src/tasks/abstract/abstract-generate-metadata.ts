/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-generate-metadata.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fs from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as remote_hsp_utils_js_1 from "../../utils/remote-hsp-utils";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as sign_util_js_1 from "../sign/sign-util";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
var __decorate = this && this.__decorate || function (e, t, a, r) {
  var s,
    o = arguments.length,
    i = o < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, a) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) i = Reflect.decorate(e, t, a, r);else for (var n = e.length - 1; n >= 0; n--) (s = e[n]) && (i = (o < 3 ? s(i) : o > 3 ? s(t, a, i) : s(t, a)) || i);
  return o > 3 && i && Object.defineProperty(t, a, i), i;
};
export class AbstractGenerateMetadata extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e, t) {
    super(e, t), this._log = ohos_logger_js_1.OhosLogger.getLogger(AbstractGenerateMetadata.name), this.sourceSetModel = e.getTargetData().getModuleSourceSetModel(), this.remoteHspMetaData = (0, remote_hsp_utils_js_1.getSignRemoteHspMetadata)(this.service, this.targetService.getTargetData().getProduct().name);
  }
  get artifactName() {
    var e;
    return null === (e = this.targetData.getTargetOpt().output) || undefined === e ? undefined : e.artifactName;
  }
  get relatedEntryModules() {
    return this.targetData.getRelatedEntryModules();
  }
  get targetDeviceType() {
    return this.targetData.getTargetDeviceType();
  }
  get isSigned() {
    return (0, sign_util_js_1.isSigned)(this.projectModel);
  }
  get outputFilePath() {
    return this.moduleModel.isHspModule() ? this.pathInfo.getIntermediatesHspOutputMetadata() : this.pathInfo.getIntermediatesOutputMetadata();
  }
  doTaskAction() {
    if (this.moduleModel.isHarModule() && this.targetName !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) return;
    const e = this.generateTargetMetadata(this.targetData);
    fs.outputJSONSync(this.outputFilePath, e);
  }
  initTaskDepends() {}
  generateTargetMetadata(e) {
    const t = this.isSigned,
      a = [],
      r = e.getModuleModel().getModuleType();
    switch (r) {
      case module_type_enum_js_1.ModuleType.Entry:
        a.push(this.generateModuleMetadata(e, t));
        break;
      case module_type_enum_js_1.ModuleType.Feature:
        if (module_task_service_js_1.ModuleTaskService.checkEntryModules(r, this.relatedEntryModules)) for (const r of this.relatedEntryModules) a.push(this.generateFeatureMetadata(e, r, t));else a.push(this.generateModuleMetadata(e, t));
        break;
      case module_type_enum_js_1.ModuleType.Shared:
        a.push(this.generateHspMetadata(e, t));
        break;
      case module_type_enum_js_1.ModuleType.Har:
        a.push(this.generateHarMetadata(e, t));
    }
    return a;
  }
  generateModuleMetadata(e, t) {
    const a = this.service.getModuleModel().getName(),
      r = e.findRelatedTargetData(a),
      s = null == r ? undefined : r.getTargetName();
    let o = "",
      i = [];
    return e.getTargetDeviceTypeClasses().forEach(n => {
      const d = n !== device_type_const_js_1.DeviceTypeConst.LITE || e.isSingleDeviceTypeTarget() ? "" : `-${n}`,
        u = t ? "signed" : "unsigned",
        _ = this.artifactName;
      o = _ ? `${_}${t ? "" : "-unsigned"}${build_directory_const_js_1.BuildArtifactExtension.DOT_HAP}` : `${a}-${s}${d}-${u}${build_directory_const_js_1.BuildArtifactExtension.DOT_HAP}`, i = null == r ? undefined : r.getTargetDeviceType().filter(e => n === device_type_const_js_1.DeviceTypeConst.LITE ? device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e) : device_type_const_js_1.DeviceTypeConst.RICH_DEVICES.includes(e));
    }), {
      hapName: o,
      deviceTypes: i,
      isSigned: t,
      entryModule: undefined,
      dependRemoteHsps: this.remoteHspMetaData
    };
  }
  generateFeatureMetadata(e, t, a) {
    var r;
    const s = e.getTargetName(),
      o = e.getModuleModel().getName(),
      i = this.artifactName,
      n = i ? `${i}${a ? "" : "-unsigned"}${build_directory_const_js_1.BuildArtifactExtension.DOT_HAP}` : `${o}-${t}-${s}-${a ? "signed" : "unsigned"}${build_directory_const_js_1.BuildArtifactExtension.DOT_HAP}`,
      d = this.targetDeviceType,
      u = this.targetData.findRelatedTargetData(t);
    return {
      hapName: n,
      deviceTypes: d,
      isSigned: a,
      entryModule: {
        moduleName: t,
        distroFilter: this.getDistroFilterObj(u),
        deviceType: null !== (r = null == u ? undefined : u.getTargetDeviceType()) && undefined !== r ? r : []
      },
      dependRemoteHsps: this.remoteHspMetaData
    };
  }
  generateHspMetadata(e, t) {
    const a = e.getTargetName(),
      r = e.getModuleModel().getName(),
      s = this.artifactName;
    return {
      hspName: s ? `${s}${t ? "" : "-unsigned"}${build_directory_const_js_1.BuildArtifactExtension.DOT_HSP}` : `${r}-${a}-${t ? "signed" : "unsigned"}${build_directory_const_js_1.BuildArtifactExtension.DOT_HSP}`,
      deviceTypes: this.targetDeviceType,
      isSigned: t,
      entryModule: undefined,
      dependRemoteHsps: this.remoteHspMetaData
    };
  }
  generateHarMetadata(e, t) {
    const a = e.getTargetName(),
      r = e.getModuleModel().getName(),
      s = this.artifactName;
    return {
      hapName: s ? `${s}${t ? "" : "-unsigned"}${build_directory_const_js_1.BuildArtifactExtension.DOT_HAP}` : `${r}-${a}-${t ? "signed" : "unsigned"}${build_directory_const_js_1.BuildArtifactExtension.DOT_HAP}`,
      deviceTypes: this.targetDeviceType,
      isSigned: t,
      entryModule: undefined,
      dependRemoteHsps: this.remoteHspMetaData
    };
  }
  getRelatedEntryJsonList(e) {
    const t = [],
      a = this.moduleModel.getModuleType(),
      r = this.targetData.getRelatedEntryModules();
    if (!module_task_service_js_1.ModuleTaskService.checkEntryModules(a, r)) return [];
    const s = this.moduleModel.getParentProject().getProjectDir();
    for (const a of r) {
      const r = this.projectModel.getModuleModelByName(a).getModule().getNodeDir(),
        o = path_1.resolve(s, r, `src/main/${e}`);
      fs.existsSync(o) && t.push(o);
      const i = path_1.resolve(s, r, common_const_js_1.CommonConst.PROFILE_JSON5);
      fs.existsSync(i) && t.push(i);
    }
    return t;
  }
}
__decorate([(0, hvigor_1.Input)()], AbstractGenerateMetadata.prototype, "remoteHspMetaData", undefined), __decorate([(0, hvigor_1.Input)()], AbstractGenerateMetadata.prototype, "artifactName", null), __decorate([(0, hvigor_1.Input)()], AbstractGenerateMetadata.prototype, "relatedEntryModules", null), __decorate([(0, hvigor_1.Input)()], AbstractGenerateMetadata.prototype, "targetDeviceType", null), __decorate([(0, hvigor_1.Input)()], AbstractGenerateMetadata.prototype, "isSigned", null), __decorate([(0, hvigor_1.OutputFile)()], AbstractGenerateMetadata.prototype, "outputFilePath", null);
