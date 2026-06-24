/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-package-hap.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import path_1 from "path";
import * as packing_tool_options_js_1 from "../../builder/inner-java-command-builder/packing-tool-options";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as code_type_enum_1 from "../../enum/code-type-enum";
import * as array_util_js_1 from "../../utils/array-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_generate_metadata_js_1 from "./legacy-generate-metadata";
import * as legacy_make_pack_info_js_1 from "./legacy-make-pack-info";
import * as legacy_sign_lite_bin_js_1 from "./legacy-sign-lite-bin";
import * as legacy_syscap_transform_js_1 from "./legacy-syscap-transform";
import * as sign_type_enum_1 from "../../enum/sign-type-enum";
import * as cache_native_libs_js_1 from "../cache-native-libs";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as sign_util_1 from "../sign/sign-util";
import * as ohos_hap_task_1 from "../task/ohos-hap-task";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyPackageHap extends ohos_hap_task_1.OhosHapTask {
  declareExecutionTool() {
    return this.sdkInfo.getPackageTool();
  }
  declareExecutionCommand() {
    if (this.needPackBin) return this.getHapBinCommand().toString();
    {
      const e = [];
      for (let t = 0; t < this.allPackHapPathsObj.length; t++) e.push(this.getHapCommand(this.allPackHapPathsObj[t]).toString());
      return e.join();
    }
  }
  declareInputs() {
    return new Map().set("needPackBin", this.needPackBin);
  }
  declareInputFiles() {
    if (this.needPackBin) {
      const e = new hvigor_1.FileSet();
      return this.targetData.hasLiteDeviceInTarget() && e.addEntry(this.signUtil._signingConfig ? this.signBinPath : this.unSignBinPath, {
        isDirectory: false
      }), e;
    }
    const e = new hvigor_1.FileSet();
    return e.addEntry(this.libPath, {
      isDirectory: false
    }), this.allPackHapPathsObj.forEach(t => {
      (this.arkCodeModel || this.jsCodeModel) && e.addEntry(t.assetsPath, {
        isDirectory: false
      }), e.addEntry(t.jsonPath, {
        isDirectory: false
      }), e.addEntry(t.resourcePath, {
        isDirectory: false
      }), e.addEntry(t.indexPath, {
        isDirectory: false
      }), e.addEntry(t.packInfoPath, {
        isDirectory: false
      });
    }), fse.existsSync(this.rpcidSc) && e.addEntry(this.rpcidSc, {
      isDirectory: false
    }), e;
  }
  declareOutputFiles() {
    if (this.needPackBin) return new hvigor_1.FileSet().addEntry(this.binOutPath, {
      isDirectory: false
    });
    const e = new hvigor_1.FileSet();
    return this.allPackHapPathsObj.forEach(t => {
      e.addEntry(t.outPath, {
        isDirectory: false
      });
    }), e;
  }
  constructor(e) {
    super(e, LegacyFATask.PACKAGE_HAP), this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyPackageHap.name), this.allPackHapPathsObj = [], this.signType = sign_type_enum_1.SignTypeEnum.BIN, this.signUtil = new sign_util_1.SignUtil(this.service, this.signType, this.pathInfo, this.targetData.getProduct(), this.sdkInfo);
    const t = this.targetService.getTargetData(),
      a = t.getPathInfo();
    this.needPackBin = this.targetService.needPackBin(t), this.unSignBinPath = path_1.resolve(a.getModuleBinOutput(), t.getModuleTargetOutputFileName("", false, build_directory_const_js_1.BuildArtifactExtension.DOT_BIN)), this.signBinPath = path_1.resolve(a.getModuleBinOutput(), t.getModuleTargetOutputFileName("", true, build_directory_const_js_1.BuildArtifactExtension.DOT_BIN)), this.binOutPath = path_1.resolve(a.getModuleBuildOutputPath(), t.getModuleTargetOutputFileName("", false, undefined, this.targetData.isSingleDeviceTypeTarget() ? undefined : device_type_const_js_1.DeviceTypeConst.LITE)), this.libPath = path_1.resolve(a.getIntermediatesProcessLibs()), this.rpcidSc = path_1.resolve(a.getIntermediatesSysCap(), common_const_js_1.CommonConst.RPCID_SC), this.apkPath = path_1.resolve(a.getIntermediatesApkDir(), t.getApkName()), this.arkCodeModel = this.moduleModel.getSourceSetByTargetName(this.targetName).getCodeMap().get(code_type_enum_1.CodeType.ETS), this.jsCodeModel = this.moduleModel.getSourceSetByTargetName(this.targetName).getCodeMap().get(code_type_enum_1.CodeType.JS);
  }
  initTaskDependsForOtherModule() {
    this.targetData.getTargetDeviceTypeClasses().forEach(e => {
      this.processDeviceType(e);
    });
  }
  processDeviceType(e) {
    const t = this.service.getRelatedEntryModules(),
      a = this.service.getModuleModel().getModuleType(),
      i = (e, t) => {
        const a = this.targetData.findRelatedTargetData(e);
        this.pushEntryInfo(this.pathInfo, e, this.targetData, a, t);
      };
    module_task_service_js_1.ModuleTaskService.checkEntryModules(a, t) ? t.forEach(t => {
      this.validateRelatedModule(t, this.targetData), this.module.findModuleByName(t) && i(t, e);
    }) : i("", e);
  }
  pushEntryInfo(e, t, a, i, s) {
    let o, n, r, d;
    if (s) {
      if (device_type_const_js_1.DeviceTypeConst.LITE === s && this.needPackBin) return;
      n = path_1.resolve(e.getIntermediatesRes(), t, this.targetData.isSingleDeviceTypeTarget() || this.needPackBin || s !== device_type_const_js_1.DeviceTypeConst.LITE ? "" : device_type_const_js_1.DeviceTypeConst.LITE, common_const_js_1.CommonConst.CONFIG_JSON), r = s === device_type_const_js_1.DeviceTypeConst.LITE ? e.getInterMediatesLoaderOutLitePath() : e.getIntermediatesFaAssetsPath(this.targetData.getProduct()), o = this.targetData.isSingleDeviceTypeTarget() ? path_1.resolve(e.getModuleBuildOutputPath(), a.getModuleTargetOutputFileName(t, false)) : path_1.resolve(e.getModuleBuildOutputPath(), a.getModuleTargetOutputFileName(t, false, undefined, s));
    } else n = path_1.resolve(e.getIntermediatesRes(), t, common_const_js_1.CommonConst.CONFIG_JSON), o = path_1.resolve(e.getModuleBuildOutputPath(), a.getModuleTargetOutputFileName(t, false)), r = e.getIntermediatesFaAssetsPath(this.targetData.getProduct());
    d = device_type_const_js_1.DeviceTypeConst.LITE === s ? path_1.resolve(e.getIntermediatesLiteSource(), "assets", this.module.getName(), t, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES) : path_1.resolve(e.getIntermediatesRes(), t, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES);
    const c = {
      jsonPath: n,
      resourcePath: d,
      indexPath: path_1.resolve(e.getIntermediatesRes(), t, build_directory_const_js_1.BuildArtifactConst.RESOURCE_INDEX),
      packInfoPath: path_1.resolve(e.getModuleBuildOutputPath(), t, build_directory_const_js_1.BuildArtifactConst.PACK_INFO),
      outPath: o,
      shellApkPath: undefined,
      assetsPath: r,
      dexPath: undefined
    };
    this.allPackHapPathsObj.push(c);
  }
  initTaskDepends() {
    this.declareDepends(legacy_make_pack_info_js_1.LegacyMakePackInfo.name), this.declareDepends(cache_native_libs_js_1.CacheNativeLibs.name), this.declareDepends(task_names_js_1.TaskNames.LegacyFATask.MERGE_NODE_ASSETS.name), this.declareDepends(legacy_generate_metadata_js_1.LegacyGenerateMetadata.name), this.declareDepends(legacy_syscap_transform_js_1.LegacySyscapTransform.name), this.service.hasLiteDevice() && this.declareDepends(legacy_sign_lite_bin_js_1.LegacySignLiteBin.name);
  }
  async doTaskAction() {
    for (const e of this.targetData.getTargetDeviceTypeClasses()) device_type_const_js_1.DeviceTypeConst.LITE === e && this.needPackBin ? await this.buildHapBinBuilder() : await this.buildHapBuilder();
  }
  async buildHapBuilder() {
    for (let e = 0; e < this.allPackHapPathsObj.length; e++) {
      const t = this.allPackHapPathsObj[e];
      await this.buildHapPackageOptions(t);
    }
  }
  async buildHapPackageOptions(e) {
    await this.executeCommandList(this.getHapCommand(e));
  }
  async buildHapBinBuilder() {
    await this.executeCommandList(this.getHapBinCommand());
  }
  getHapCommand(e) {
    const t = "generate HAP packaging command",
      a = this.durationEvent.createSubEvent(t, "");
    a.start();
    const i = new packing_tool_options_js_1.PackingToolOptions();
    return i.addCalledJarFile(this.sdkInfo.getPackageTool()), this.jsCodeModel || this.arkCodeModel ? i.addMode("hap").force(true).addLibPath(this.libPath).addJsonPath(e.jsonPath).addResourcesPath(e.resourcePath).addAssetsPath(e.assetsPath).addIndexPath(e.indexPath).addPackInfoPath(e.packInfoPath).addOutPath(e.outPath) : i.addMode("hap").force(true).addLibPath(this.libPath).addJsonPath(e.jsonPath).addResourcesPath(e.resourcePath).addIndexPath(e.indexPath).addPackInfoPath(e.packInfoPath).addOutPath(e.outPath), fse.existsSync(this.rpcidSc) && i.addSysCapPath(this.rpcidSc), a.stop(), a.setLog(t, hvigor_1.MetricLogType.INFO), i.build();
  }
  getHapBinCommand() {
    const e = "generate HAP BIN packaging command",
      t = this.durationEvent.createSubEvent(e, "");
    t.start();
    const a = new packing_tool_options_js_1.PackingToolOptions();
    return a.addCalledJarFile(this.sdkInfo.getPackageTool()), this.signUtil._signingConfig ? a.addMode("hap").addBinPath(this.signBinPath).addOutPath(this.binOutPath).force(true) : a.addMode("hap").addBinPath(this.unSignBinPath).addOutPath(this.binOutPath).force(true), t.stop(), t.setLog(e, hvigor_1.MetricLogType.INFO), a.build();
  }
  async executeCommandList(e) {
    this._log._printDebugCommand("PackageHap", e);
    const t = {
        moduleName: this.moduleName,
        taskName: this.name,
        commandLine: e
      },
      a = "submit HAP packaging task to work pool",
      i = this.durationEvent.createSubEvent(a, "");
    i.start(), await new process_utils_js_1.ProcessUtils(this.moduleName, this.name).submitExecutionWithoutReturn(this, this.getWorkerPool(), t, wdk_1.noop, [], i), i.stop(), i.setLog(a, hvigor_1.MetricLogType.INFO);
  }
  validateRelatedModule(e, t) {
    var a;
    const i = t.getTargetName(),
      s = t.getModuleModel().getName(),
      o = t.getTargetDeviceType(),
      n = t.findRelatedTargetData(e),
      r = null == n ? undefined : n.getModuleModel().getName(),
      d = null == n ? undefined : n.getTargetName(),
      c = null == n ? undefined : n.getTargetDeviceType(),
      l = (0, array_util_js_1.getIntersection)(o, c),
      _ = (null == n ? undefined : n.getModuleSourceSetModel()).getLegacyModuleTargetRes().getJsonPath(),
      u = undefined === (null === (a = null == n ? undefined : n.getTargetOpt().config) || undefined === a ? undefined : a.deviceType) ? _ : null == n ? undefined : n.getModuleModel().getProfilePath();
    0 === l.length && this._log.printErrorExit("NO_DEVICE_TYPE_INTERSECTION_WITH_ENTRY_MODULES", [o, i, s, c, d, r, u], [[i]]);
  }
}
