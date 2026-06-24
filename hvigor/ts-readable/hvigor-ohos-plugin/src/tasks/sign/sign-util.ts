/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/sign-util.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import fs_extra_1 from "fs-extra";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as project_path_info_iml_js_1 from "../../common/iml/project-path-info-iml";
import * as common_const_js_1 from "../../const/common-const";
import * as sign_type_enum_js_1 from "../../enum/sign-type-enum";
import * as array_util_js_1 from "../../utils/array-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as keystore_utils_js_1 from "../../utils/keystore-utils";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as sign_command_factory_js_1 from "./command-builder-impl/sign-command-factory";
import * as sign_request_js_1 from "./sign-request";
export class SignUtil {
  constructor(e, t, i, n, s) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(SignUtil.name);
    const o = e.getProjectModel().getCompileApiMetaByProduct(n.name).version;
    this._taskService = e, this._signType = t, this._targetProduct = n, this._signCommandFactory = new sign_command_factory_js_1.SignCommandFactory(o), this._pathInfo = i, this.sdkInfo = s;
  }
  get _signingConfig() {
    return this.getSigningConfig();
  }
  get _signingConfigCheckLogStr() {
    return this.signingConfigCheckLogStr = undefined, this.getSigningConfig(), this.signingConfigCheckLogStr;
  }
  async sign(e, t) {
    var i;
    if (e.getSignType() !== sign_type_enum_js_1.SignTypeEnum.HAR) {
      if (inject_util_js_1.InjectUtil.isDaemon() && this.sdkInfo.supportHapSignToolApiCall(!this.sdkInfo.isOhos)) try {
        await this.executeSignRequest(e, t);
      } catch (i) {
        this._log.debug(`java daemon remote sign failed ${null == i ? undefined : i.code} fallback to local java -jar sign`), await this.executeSign(e, t);
      } else await this.executeSign(e, t);
      fs_extra_1.existsSync(e.getOutPutFilePath()) || e.getSignType() !== sign_type_enum_js_1.SignTypeEnum.SHELL || fs_extra_1.copyFileSync(e.getInputFilePath(), e.getOutPutFilePath());
    } else try {
      await this.executeSignRequest(e, t);
    } catch (e) {
      if (hvigor_logger_1.ErrorUtil.isAdaptorError(e)) throw e;
      this._log.errorMessageExit(`Remote sign har failed. errorCode: ${null !== (i = null == e ? undefined : e.code) && undefined !== i ? i : ""}`);
    }
  }
  getSignCommand(e) {
    if (!this._signingConfig) return;
    return this._signCommandFactory.createCommandBuilder(this._taskService.getProjectModel(), this._signingConfig, this.sdkInfo, e).getSignCommand();
  }
  getBundleNameFromHap(e) {
    return validate_util_js_1.ValidateUtil.getBundleNameFromHap(this._targetProduct, e);
  }
  getVerifySignCommands(e) {
    return validate_util_js_1.ValidateUtil.getVerifySignCommands(this.sdkInfo, this._signingConfig, e);
  }
  async validateBundleName(e, t, i, n = false) {
    const s = this._taskService.getProjectModel();
    if (!(t instanceof project_path_info_iml_js_1.ProjectPathInfoIml || (null == e ? undefined : e.getSignType()) !== sign_type_enum_js_1.SignTypeEnum.HAP && (null == e ? undefined : e.getSignType()) !== sign_type_enum_js_1.SignTypeEnum.HOS_HAP)) {
      const e = await validate_util_js_1.ValidateUtil.getBundleNameFromP7b(this.sdkInfo, this._signingConfig, t, i, n);
      if (validate_util_js_1.ValidateUtil.getBundleNameFromHap(this._targetProduct, s) !== e) {
        const e = this._taskService.isFaMode() ? common_const_js_1.CommonConst.CONFIG_JSON : common_const_js_1.CommonConst.APP_CONFIG;
        this._log.printErrorExit("BUNDLE_NAME_DOES_NOT_MATCH", [e, this._taskService.getProjectModel().getProfilePath()], [[], [e]]);
      }
    }
  }
  async executeSign(e, t) {
    const i = this._signingConfig;
    if (!i) return;
    const n = `generate ${e.getSignType()} signing command`,
      s = t.createSubEvent(n, "");
    s.start();
    const o = this._signCommandFactory.createCommandBuilder(this._taskService.getProjectModel(), i, this.sdkInfo, e).getSignCommand(),
      g = `execute ${e.getSignType()} signing command`;
    if ((0, wdk_1.isEqual)(i, SignUtil.getDefaultSign())) {
      s.stop(), s.setLog(n, hvigor_1.MetricLogType.INFO);
      const i = t.createSubEvent(g, "");
      return i.start(), await new process_utils_js_1.ProcessUtils(this._taskService.getNode().getName(), `Sign${e.getSignType()}`).execute(o), i.stop(), void i.setLog(g, hvigor_1.MetricLogType.INFO);
    }
    await this.validateBundleName(e, this._pathInfo, s), s.stop(), s.setLog(n, hvigor_1.MetricLogType.INFO);
    const r = t.createSubEvent(g, "");
    r.start(), this._taskService instanceof module_task_service_js_1.ModuleTaskService ? await new process_utils_js_1.ProcessUtils(this._taskService.getModuleModel().getName(), `Sign${e.getSignType()}`).execute(o) : await new process_utils_js_1.ProcessUtils(this._taskService.getProjectModel().getName(), `Sign${e.getSignType()}`).execute(o), r.stop(), r.setLog(g, hvigor_1.MetricLogType.INFO);
  }
  async executeSignRequest(e, t) {
    const i = this._signingConfig;
    if (!i) return;
    const n = `generate ${e.getSignType()} signing command`,
      s = t.createSubEvent(n, "");
    s.start();
    const o = this._signCommandFactory.createCommandBuilder(this._taskService.getProjectModel(), i, this.sdkInfo, e),
      g = (o.getSignCommand(), `execute ${e.getSignType()} signing command`),
      r = async () => {
        if (e.getSignType() === sign_type_enum_js_1.SignTypeEnum.HAR) {
          const t = (0, sign_request_js_1.constructHarSignParams)(i, e.getInputFilePath(), e.getOutPutFilePath(), o.readKeyStorePwd(), o.readKeyPwd());
          await (0, sign_request_js_1.sendSignHarRequest)(o.getSignTool(), t);
        } else {
          const t = e.getSignType() === sign_type_enum_js_1.SignTypeEnum.BIN ? "bin" : "zip",
            n = (0, sign_request_js_1.constructSignParams)(i, e.getInputFilePath(), e.getOutPutFilePath(), o.readKeyStorePwd(), o.readKeyPwd(), t);
          await (0, sign_request_js_1.sendSignRequest)(o.getSignTool(), n);
        }
      };
    if ((0, wdk_1.isEqual)(i, SignUtil.getDefaultSign())) {
      s.stop(), s.setLog(n, hvigor_1.MetricLogType.INFO);
      const e = t.createSubEvent(g, "");
      return e.start(), await r(), e.stop(), void e.setLog(g, hvigor_1.MetricLogType.INFO);
    }
    await this.validateBundleName(e, this._pathInfo, s, true), s.stop(), s.setLog(n, hvigor_1.MetricLogType.INFO);
    const a = t.createSubEvent(g, "");
    a.start(), await r(), a.stop(), a.setLog(g, hvigor_1.MetricLogType.INFO);
  }
  getSigningConfig() {
    var e;
    const t = this._taskService.getProjectModel().getProfileOpt().app,
      i = this._targetProduct.signingConfig,
      n = (0, wdk_1.cloneDeep)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.signingConfig);
    if (undefined === i) return n || void (this.signingConfigCheckLogStr = `No signingConfig found for product ${this._targetProduct.name}`);
    const s = (0, array_util_js_1.getElementFromArr)(null == t ? undefined : t.signingConfigs, i.substring(i.lastIndexOf(".") + 1));
    return undefined === s ? n || void this.signingConfigCheck(i) : mergeSigningConfig(s, n);
  }
  signingConfigCheck(e) {
    e.includes(".") ? this.signingConfigCheckLogStr = `Invalid signingConfig value detected in product '${this._targetProduct.name}'. Signing '${this._signType}' will be skipped.\n        Ensure that the signingConfig value contains only numbers and letters, with a maximum of 64 characters.\n        Path: ${this._taskService.getProjectModel().getProfilePath()}` : this.signingConfigCheckLogStr = `Will skip sign '${this._signType}'. No signingConfigs profile is configured in current project.\n             If needed, configure the signingConfigs in ${this._taskService.getProjectModel().getProfilePath()}.`;
  }
  static getDefaultSign() {
    return {
      material: {
        storeFile: keystore_utils_js_1.KeyStoreHelper.getDefaultDebugKeyStoreLocation(),
        storePassword: keystore_utils_js_1.KeyStoreHelper.DEFAULT_STORE_PASS,
        keyAlias: keystore_utils_js_1.KeyStoreHelper.DEFAULT_ALIAS,
        keyPassword: keystore_utils_js_1.KeyStoreHelper.DEFAULT_STORE_PASS,
        signAlg: "",
        profile: "",
        certpath: ""
      },
      name: ""
    };
  }
  static isUseDefaultShellSign(e) {
    const t = SignUtil.getDefaultSign().material;
    return e.storePassword === t.storePassword && e.keyPassword === t.keyPassword && e.storeFile === t.storeFile && e.keyAlias === t.keyAlias;
  }
  static getSigningConfigInputs(e, t) {
    var i, n, s, o, g, r;
    const a = new Map();
    if (a.set(common_const_js_1.CommonConst.ENABLE_SIGN_TASK, hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.ENABLE_SIGN_TASK)), a.set("sdkToolchainsComponentVersion", e.getToolchainsComponentVersion()), !t) return a.set("existSigningConfig", false), a;
    a.set("existSigningConfig", true).set("signingConfig_name", null !== (i = t.name) && undefined !== i ? i : "").set("signingConfig_type", null !== (n = t.type) && undefined !== n ? n : "");
    const _ = t.material;
    return _ ? (a.set("existMaterial", true).set("signingConfig_signAlg", null !== (s = _.signAlg) && undefined !== s ? s : "").set("signingConfig_keyAlias", null !== (o = _.keyAlias) && undefined !== o ? o : "").set("signingConfig_keyPassword", null !== (g = _.keyPassword) && undefined !== g ? g : "").set("signingConfig_storePassword", null !== (r = _.storePassword) && undefined !== r ? r : ""), a) : (a.set("existMaterial", false), a);
  }
  getSigningConfigInputFiles() {
    var e;
    const t = new hvigor_1.FileSet(),
      i = null === (e = this._signingConfig) || undefined === e ? undefined : e.material;
    return (null == i ? undefined : i.certpath) && fs_extra_1.existsSync(null == i ? undefined : i.certpath) && t.addEntry(file_util_js_1.FileUtil.convertToAbsolutePath(i.certpath, this._taskService.getProjectModel().getProjectDir()), {
      isDirectory: false
    }), (null == i ? undefined : i.profile) && fs_extra_1.existsSync(null == i ? undefined : i.profile) && t.addEntry(file_util_js_1.FileUtil.convertToAbsolutePath(i.profile, this._taskService.getProjectModel().getProjectDir()), {
      isDirectory: false
    }), (null == i ? undefined : i.storeFile) && fs_extra_1.existsSync(null == i ? undefined : i.storeFile) && t.addEntry(file_util_js_1.FileUtil.convertToAbsolutePath(i.storeFile, this._taskService.getProjectModel().getProjectDir()), {
      isDirectory: false
    }), t;
  }
}
export const isSigned = (e, t = false) => !!getSigningConfig(e) && (t ? !!hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.HvigorConfigConst.OHOS_SIGN_HAR) : !!hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.ENABLE_SIGN_TASK));
export function getSigningConfig(e) {
  var t;
  const i = (0, find_target_product_js_1.findTargetProduct)(e),
    n = e.getProfileOpt().app,
    s = i.signingConfig,
    o = (0, wdk_1.cloneDeep)(null === (t = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === t ? undefined : t.signingConfig);
  if (undefined === s) return o;
  const g = (0, array_util_js_1.getElementFromArr)(null == n ? undefined : n.signingConfigs, s.substring(s.lastIndexOf(".") + 1));
  return undefined === g ? o : mergeSigningConfig(g, o);
}
export function mergeSigningConfig(e, t) {
  var i, n, s;
  return {
    material: {
      ...e.material,
      ...(null !== (i = null == t ? undefined : t.material) && undefined !== i ? i : {})
    },
    name: null !== (n = e.name) && undefined !== n ? n : null == t ? undefined : t.name,
    type: null !== (s = e.type) && undefined !== s ? s : null == t ? undefined : t.type
  };
}
export function getHqfName(e, t, i = false) {
  return `${e}-${t}-${i ? "signed" : "unsigned"}.hqf`;
}
