/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-packages-from-app.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_config_loader_js_1 from "@ohos/hvigor/src/common/util/hvigor-config-loader";
import fs_extra_1 from "fs-extra";
import * as packing_tool_options_js_1 from "../builder/inner-java-command-builder/packing-tool-options";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as sign_type_enum_js_1 from "../enum/sign-type-enum";
import * as file_util_js_1 from "../utils/file-util";
import * as process_utils_js_1 from "../utils/process-utils";
import * as base_pack_app_task_js_1 from "./base/base-pack-app-task";
import * as task_names_js_1 from "./common/task-names";
import * as package_app_js_1 from "./package-app";
import * as sign_model_js_1 from "./sign/command-builder-impl/sign-model";
import * as sign_util_js_1 from "./sign/sign-util";
import * as sign_job_js_1 from "./worker/sign-job";
var Task = task_names_js_1.TaskNames.Task;
export class SignPackagesFromApp extends base_pack_app_task_js_1.BasePackAppTask {
  declareInputs() {
    return sign_util_js_1.SignUtil.getSigningConfigInputs(this.service.getSdkInfo(), this.signingConfig).set("appWithSignedPkg", this.appWithSignedPkg).set("isSigned", this.isSigned);
  }
  declareInputFiles() {
    let e;
    return e = this.signUtil ? this.signUtil.getSigningConfigInputFiles() : new hvigor_1.FileSet(), fs_extra_1.existsSync(this.appFilePath) && e.addEntry(this.appFilePath), e;
  }
  declareOutputFiles() {
    const e = super.declareInputFiles();
    return this.appWithSignedPkg && e.addEntry(this.allAppFilePath), e;
  }
  constructor(e, t) {
    super(e, t, Task.SIGN_PACKAGES_FROM_APP), this.defaultPackageLimitSize = 2, this.curProduct = t.getTargetProduct(), this.unPackTool = this.service.getSdkInfo().getUnPackageTool();
    const s = this.curProduct.runtimeOS === common_const_js_1.CommonConst.HARMONY_OS;
    this.moduleSignType = s ? sign_type_enum_js_1.SignTypeEnum.HOS_HAP : sign_type_enum_js_1.SignTypeEnum.HAP, this.signUtil = new sign_util_js_1.SignUtil(this.service, this.moduleSignType, this.pathInfo, this.service.getTargetProduct(), this.service.getSdkInfo()), this.signingConfig = this.signUtil._signingConfig, this.isSigned = (0, sign_util_js_1.isSigned)(this.service.getProjectModel()), this.appFilePath = path_1.resolve(this.pathInfo.getProjectOutputPath(), this.service.getAppOutputFileName(false)), this.allAppFilePath = path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), this.service.getAllAppOutputFileName()), this.allSignedAppFilePath = path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), this.service.getAllAppOutputFileName(true)), this.pacJsonFilePath = path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), common_const_js_1.CommonConst.PAC_ONLY_JSON);
  }
  initTaskDepends() {
    this.dependsOn(package_app_js_1.PackageApp.name);
  }
  get appWithSignedPkg() {
    var e, t;
    return null !== (t = null === (e = this.service.getBuildOption().packOptions) || undefined === e ? undefined : e.appWithSignedPkg) && undefined !== t && t;
  }
  async doTaskAction() {
    if (this.deleteOldAllAppPackages(), !fs_extra_1.existsSync(this.appFilePath) || !this.appWithSignedPkg || !this.isSigned) return;
    const e = await this.unpackAppPackage(),
      {
        hapFileList: t,
        hspFileList: s
      } = await this.signAllHapsAndHsps(e);
    await this.packAllAppPackage(t, s), fs_extra_1.existsSync(e) && fs_extra_1.rmSync(e, {
      recursive: true,
      force: true
    });
  }
  async packAllAppPackage(e, t) {
    const s = "package all-app",
      i = this.durationEvent.createSubEvent(s, "");
    i.start();
    const a = this.getPackAppCommand(e, t);
    await new process_utils_js_1.ProcessUtils().execute(a), i.stop(), i.setLog(s, hvigor_1.MetricLogType.INFO);
  }
  async signAllHapsAndHsps(e) {
    const t = "sign all packages",
      s = this.durationEvent.createSubEvent(t, "");
    s.start();
    const i = fs_extra_1.readdirSync(e),
      a = [],
      n = [],
      o = [];
    for (const t of i) {
      const s = path_1.resolve(e, t);
      if (t.endsWith(build_directory_const_js_1.BuildArtifactExtension.DOT_HAP) || t.endsWith(build_directory_const_js_1.BuildArtifactExtension.DOT_HSP)) {
        const i = path_1.extname(t),
          r = path_1.basename(t),
          p = path_1.resolve(e, "signed");
        fs_extra_1.ensureDirSync(p);
        const l = path_1.resolve(p, r);
        i === build_directory_const_js_1.BuildArtifactExtension.DOT_HAP ? a.push(l) : n.push(l);
        const c = new sign_model_js_1.SignModel(this.moduleSignType, s, l);
        o.push(c);
      }
    }
    const r = [];
    for (const e of o) r.push(this.submitSignWorker(e));
    return await Promise.all(r), s.stop(), s.setLog(t, hvigor_1.MetricLogType.INFO), {
      hapFileList: a,
      hspFileList: n
    };
  }
  async submitSignWorker(e) {
    return new Promise((t, s) => {
      const i = this.signUtil.getSignCommand(e);
      if (!i) return void this._log.debug("No signCommand, please check sign info.");
      const a = {
          signCommand: i,
          projectName: this.service.getProjectModel().getName(),
          signType: e.getSignType()
        },
        n = path_1.resolve(__dirname, "./worker/sign-job.js/signPackageFromApp"),
        o = {
          workInput: a,
          callback: () => {
            t();
          }
        };
      if (this.getWorkerPool().submit(this, n, o).getState() === hvigor_1.TaskState.REJECT) {
        const e = "sign packages from app",
          t = this.durationEvent.createSubEvent(e, "");
        t.start(), this._log.debug("Worker dispatch failed, sign packages from app on main thread."), (0, sign_job_js_1.signPackageFromApp)(a).catch(() => {
          s(new Error("Sign package from app failed!"));
        }), t.stop(), t.setLog(e, hvigor_1.MetricLogType.INFO);
      }
    });
  }
  async unpackAppPackage() {
    const e = "unzip app package",
      t = this.durationEvent.createSubEvent(e, "");
    t.start();
    const s = file_util_js_1.FileUtil.getRemoveExtPath(this.appFilePath);
    fs_extra_1.existsSync(s) && fs_extra_1.rmSync(s, {
      recursive: true,
      force: true
    });
    const i = new packing_tool_options_js_1.PackingToolOptions();
    i.addCalledJarFile(this.unPackTool), i.addMode("app").addAppPath(this.appFilePath).addOutPath(s).force(true);
    const a = i.build();
    return await new process_utils_js_1.ProcessUtils().execute(a), t.stop(), t.setLog(e, hvigor_1.MetricLogType.INFO), s;
  }
  getPackAppCommand(e, t) {
    const s = new packing_tool_options_js_1.PackingToolOptions();
    s.addCalledJarFile(this.packageTool), this.needPackRes && fs_extra_1.existsSync(this.packResPath) && s.addPackResPath(this.packResPath), s.addMode("app").addPackInfoPath(this.packInfoPath).addHapPath(e.join(",")).addHspPath(t.join(",")).force(true).replacePackInfo(false).addOutPath(this.allAppFilePath).addMainModuleLimit(this.defaultPackageLimitSize).addNormalModuleLimit(this.defaultPackageLimitSize);
    const i = hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.HvigorConfigConst.OHOS_PACK_COMPRESS_LEVEL);
    return i && s.addCompressLevel(i, this.service.getSdkInfo().getSdkVersion()), fs_extra_1.existsSync(this.pacJsonFilePath) && s.addPacArg(this.pacJsonFilePath), s.build();
  }
  deleteOldAllAppPackages() {
    fs_extra_1.existsSync(this.allAppFilePath) && fs_extra_1.removeSync(this.allAppFilePath), fs_extra_1.existsSync(this.allSignedAppFilePath) && fs_extra_1.removeSync(this.allSignedAppFilePath);
  }
}
