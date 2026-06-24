/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/validate/validate-util.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_config_loader_js_1 from "@ohos/hvigor/src/common/util/hvigor-config-loader";
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import * as fs from "fs-extra";
import * as os from "os";
import path_1 from "path";
import * as util from "util";
import * as java_command_builder_js_1 from "../../builder/java-command-builder";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as common_const_js_1 from "../../const/common-const";
import * as version_const_js_1 from "../../const/version-const";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as sdk_util_js_1 from "../../sdk/sdk-util";
import * as array_util_js_1 from "../array-util";
import * as file_util_js_1 from "../file-util";
import * as ohos_logger_js_1 from "../log/ohos-logger";
import * as process_utils_js_1 from "../process-utils";
import * as validator_store_js_1 from "./validator-store";
import * as const_1 from "@ohos/hvigor/src/common/const/const";
import * as path_const_1 from "@ohos/hvigor/src/common/const/path-const";
import * as dependency_interface_1 from "../../project/dependency/core/dependency-interface";
import * as sign_request_js_1 from "../../tasks/sign/sign-request";
import * as json_util_js_1 from "../json-util";
import * as npm_utils_1 from "../npm-utils";
export class ValidateUtil {
  static modelVersionCheck() {
    var e;
    const i = null === (e = hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getHvigorConfig()) || undefined === e ? undefined : e.modelVersion,
      t = path_1.resolve(path_const_1.HVIGOR_PROJECT_ROOT_DIR, const_1.DEFAULT_OH_PACKAGE_JSON_FILE_NAME),
      o = path_1.resolve(path_const_1.HVIGOR_PROJECT_ROOT_DIR, const_1.HVIGOR, const_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME),
      r = hvigor_1.ParseJsonFile.parseJsonFile(t, false).modelVersion;
    i && r ? ("string" != typeof i && this._log.printErrorExit("INCOMPATIBLE_TYPES_OF_HVIGOR_MODEL_VERSION", [o], [[typeof i]]), "string" != typeof r && this._log.printErrorExit("INCOMPATIBLE_TYPES_OF_OH_MODEL_VERSION", [t], [[typeof r]]), i === r ? (((0, sdkmanager_common_1.compareVersion)(i, version_const_js_1.VersionConst.CURRENT_MODEL_VERSION) > 0 || (0, sdkmanager_common_1.compareVersion)(i, version_const_js_1.VersionConst.MINIMUM_MODEL_VERSION) < 0) && this._log.printErrorExit("UNSUPPORTED_MODEL_VERSION_OF_HVIGOR", [i], [[version_const_js_1.VersionConst.CURRENT_MODEL_VERSION]]), ((0, sdkmanager_common_1.compareVersion)(r, version_const_js_1.VersionConst.CURRENT_MODEL_VERSION) > 0 || (0, sdkmanager_common_1.compareVersion)(r, version_const_js_1.VersionConst.MINIMUM_MODEL_VERSION) < 0) && this._log.printErrorExit("UNSUPPORTED_MODEL_OH_PKG_OF_HVIGOR", [i, t], [[version_const_js_1.VersionConst.CURRENT_MODEL_VERSION]])) : this._log.printErrorExit("INCONSISTENT_MODEL_VERSION", [i, r, t])) : this._log.printErrorExit("PROJECT_STRUCTURE_AND_CONFIGURATION");
  }
  static integrationVersionCheck(e, i, t, o) {
    if (e) {
      if (i.compileSdkVersion) {
        const t = (0, sdk_util_js_1.parseApiVersion)(i.compileSdkVersion, e),
          r = new sdkmanager_common_1.ApiVersion(t.fullVersion),
          n = (0, sdk_util_js_1.parseApiVersion)(version_const_js_1.VersionConst.SUPPORT_COMPILE_VERSION, e),
          s = new sdkmanager_common_1.ApiVersion(n.fullVersion);
        (0, sdkmanager_common_1.isEqualApiVersion)(r, s) && 0 === (0, sdkmanager_common_1.compareVersion)(t.api, n.api) || this._log.printErrorExit("UNSUPPORTED_COMPILESDKVERSION", [i.compileSdkVersion, o], [[version_const_js_1.VersionConst.SUPPORT_COMPILE_VERSION]]);
      }
      if (i.compatibleSdkVersion) {
        const r = (0, sdk_util_js_1.parseApiVersion)(i.compatibleSdkVersion, e),
          n = t ? version_const_js_1.VersionConst.ATOMIC_SERVICE_MINIMUM_COMPATIBLE_VERSION : version_const_js_1.VersionConst.MINIMUM_COMPATIBLE_VERSION,
          s = (0, sdk_util_js_1.parseApiVersion)(n, e),
          a = new sdkmanager_common_1.ApiVersion(r.fullVersion),
          l = new sdkmanager_common_1.ApiVersion(s.fullVersion);
        ((0, sdkmanager_common_1.compareApiVersion)(a, l) < 0 || (0, sdkmanager_common_1.compareVersion)(r.api, s.api) < 0) && this._log.printErrorExit("UNSUPPORTED_COMPATIBLESDKVERSION", [i.compileSdkVersion, o], [[n, version_const_js_1.VersionConst.SUPPORT_COMPILE_VERSION]]);
      }
    } else i.compileSdkVersion || this._log.printErrorExit("OPENHARMONY_WITHOUT_COMPILESDKVERSION", [o]);
  }
  static apiConfigurationCheck(e) {
    const i = (0, find_target_product_js_1.findTargetProduct)(e);
    i.compatibleSdkVersion || this._log.printErrorExit("NO_COMPATIBLESDKVERSION", [e.getProfilePath()]), i.runtimeOS !== common_const_js_1.CommonConst.OPEN_HARMONY && "number" != typeof i.compatibleSdkVersion || i.compileSdkVersion || this._log.printErrorExit("NO_COMPILESDKVERSION_IN_OPENHARMONY", [e.getProfilePath()]);
  }
  static checkProducts(e) {
    return !(!e.compileSdkVersion && !e.compatibleSdkVersion);
  }
  static apiCompatibleCheck(e, i, t) {
    if ((null == t ? undefined : t.getApiType()) === hap_extra_info_js_1.ApiType.FA && i.getCompatibleApiVersion() <= 7) return {
      suggestedVersion: 8,
      errorHandler: () => {
        this._log.printErrorExit("FA_COMPATIBLESDKVERSION", [], [[null == e ? undefined : e.getProfilePath()]]);
      }
    };
    const o = {
      suggestedVersion: -1
    };
    if ((null == t ? undefined : t.getApiType()) !== hap_extra_info_js_1.ApiType.STAGE && !i.isHarmonyOS()) return o;
    if ((null == t ? undefined : t.getApiType()) === hap_extra_info_js_1.ApiType.STAGE && i.getCompatibleApiVersion() <= 8) return {
      suggestedVersion: 9,
      errorHandler: () => {
        this._log.printErrorExit("STAGE_COMPATIBLESDKVERSION", [], [[null == e ? undefined : e.getProfilePath()]]);
      }
    };
    const r = this.hosApiMatrixCheck(i);
    return r[0] ? o : {
      suggestedVersion: r[1],
      errorHandler: () => {
        this._log.printErrorExit("API_COMPATIBLESDKVERSION", [i.getCompileApiVersion(), r[1]], [[r[1], null == e ? undefined : e.getProfilePath()]]);
      }
    };
  }
  static hosApiMatrixCheck(e) {
    if (!e.isHarmonyOS()) return [true, e.getApiMeta().compatibleSdkVersion.version];
    const i = this.hosApiCompatible.get(e.getApiMeta().compileSdkVersion.version);
    return i ? [e.getApiMeta().compatibleSdkVersion.version >= i, i] : [true, -1];
  }
  static schemaCheckOnly(e, i, t) {
    return t(json_util_js_1.JsonUtil.getJson5Obj(i));
  }
  static submitSchemaCheckWork(e) {
    const {
        moduleName: i,
        filePath: t,
        schemaPath: o,
        changeAppSchema: r = false,
        checkOnly: n = false,
        errorHandlers: s
      } = e,
      a = validator_store_js_1.ValidatorStore.addValidator(o, r);
    return n ? ValidateUtil.schemaCheckOnly(i, t, a) : ValidateUtil.doSchemaCheck(i, t, a, s);
  }
  static schemaCheckJSObject(e, i, t) {
    var o;
    const r = t(e);
    if (!r) {
      if (!t.errors) return r;
      const e = [];
      if (null === (o = t.errors) || undefined === o || o.forEach(t => {
        const o = {
          instancePath: t.instancePath,
          keyword: t.keyword,
          params: t.params,
          message: t.message
        };
        i && (o.location = `${null == i ? undefined : i.replace(/\\/g, "/")}`), e.push(o);
      }), 0 !== e.length) {
        let t = "";
        null == e || e.forEach(e => {
          t += util.format(e, os.EOL);
        });
        const o = i ? `Schema validate failed, at file: ${i}` : "Schema validate failed.";
        ValidateUtil._log.printErrorExit("SCHEMA_VALIDATE_FAILED", [o], [["Please check the following fields.", os.EOL, t]]);
      }
    }
    return r;
  }
  static doSchemaCheck(e, i, t, o) {
    var r;
    const n = json_util_js_1.JsonUtil.getJson5Obj(i),
      s = t(n);
    if (!s) {
      if (!t.errors) return s;
      const e = hvigor_1.ParseJsonFile.parseJsonFile(i, true);
      let a = [];
      if (null === (r = t.errors) || undefined === r || r.forEach(t => {
        const o = {
          instancePath: t.instancePath,
          keyword: t.keyword,
          params: t.params,
          message: t.message,
          location: `${i.replace(/\\/g, "/")}`
        };
        ValidateUtil.addLocationInfo(o, e), a.push(o);
      }), o && o.forEach(e => {
        a = e(a, n);
      }), 0 !== a.length) {
        let e = "";
        null == a || a.forEach(i => {
          e += util.format(i, os.EOL);
        });
        const t = `Schema validate failed, at file: ${i}`;
        ValidateUtil._log.printErrorExit("SCHEMA_VALIDATE_FAILED", [t], [["Please check the following fields.", os.EOL, e]]);
      }
    }
    return s;
  }
  static addLocationInfo(e, i) {
    var t;
    let o = "",
      r = i;
    if (null === (t = e.instancePath) || undefined === t || t.split("/").forEach(e => {
      null === e || "" === e || ("NaN" === parseInt(e).toString() ? (r = r[e], o += "" === o ? e : `.${e}`) : (r = r[parseInt(e)], o += `[${e}]`));
    }), e.instancePath = o, undefined === r._line) {
      const i = ValidateUtil.findLocationByPath(e.location, o);
      i && (e.location += `:${null == i ? undefined : i._line}:${null == i ? undefined : i._column}`);
    } else e.location += `:${r._line}:${r._column + 1}`;
  }
  static findLocationByPath(e, i) {
    const t = i.replace(/\[(\d+)\]/g, ".$1").split(".").filter(e => "" !== e).map(e => e.match(/^\d+$/) ? Number(e) : e);
    if (0 === t.length) return;
    const o = t[t.length - 1];
    let r = null;
    for (let e = 0; e < t.length - 1; e++) if ("string" == typeof t[e] && "number" == typeof t[e + 1] && t.slice(e + 2).includes(o)) {
      r = t[e + 1];
      break;
    }
    const n = fs.readFileSync(e, "utf8").split(/\r?\n/),
      s = `"${o}"`;
    let a = 0;
    for (let e = 0; e < n.length; e++) {
      const i = n[e];
      if (i.includes(s)) {
        if (null === r || a === r) {
          return {
            _line: e + 1,
            _column: i.indexOf(s) + 2 + s.length
          };
        }
        a++;
      }
    }
  }
  static async getBundleNameFromP7b(e, i, t, o, r = false) {
    var n, s;
    if (undefined !== i) {
      const {
          verifyTool: a,
          signingConfigP7bFile: l,
          signConfigCheckJsonFile: _
        } = this.getVerifySignInfo(e, i, t),
        c = "verify signing configuration and get bundle name",
        d = o.createSubEvent(c, "");
      d.start(), r ? await this.executeVerifySingningConfigRequest(a, l, _) : await this.executeVerifySingningConfig(a, l, _), d.stop(), d.setLog(c, hvigor_1.MetricLogType.INFO);
      const u = null === (s = null === (n = json_util_js_1.JsonUtil.getJson5Obj(_).content) || undefined === n ? undefined : n["bundle-info"]) || undefined === s ? undefined : s["bundle-name"];
      try {
        fs.existsSync(_) && fs.removeSync(_);
      } catch (e) {
        e instanceof Error && this._log.printErrorExit("DELETE_FILE_ERROR", [_]);
      }
      return u;
    }
    return null;
  }
  static getVerifySignInfo(e, i, t) {
    const o = e.getVerifySignConfigTool();
    "" !== o && fs.existsSync(path_1.resolve(o)) || ValidateUtil._log.printErrorExit("NO_HAP_SIGNING_TOOL", [], [[e.getSdkToolchainsDir()]]);
    const r = i.material.profile,
      n = t.getIntermediatesTemp(),
      s = path_1.resolve(n, "signConfigCheckJson.json");
    return file_util_js_1.FileUtil.checkDirWithoutDelete(path_1.resolve(n)), file_util_js_1.FileUtil.checkFile(s), {
      verifyTool: o,
      signingConfigP7bFile: r,
      signConfigCheckJsonFile: s
    };
  }
  static getVerifySignCommands(e, i, t) {
    if (undefined !== i) {
      const {
        verifyTool: o,
        signingConfigP7bFile: r,
        signConfigCheckJsonFile: n
      } = this.getVerifySignInfo(e, i, t);
      return new java_command_builder_js_1.JavaCommandBuilder().addCalledJarFile(o).addJvmOption("verify-profile").addJvmOption("-inFile").addJvmOption(r).addJvmOption("-outFile").addJvmOption(n).build();
    }
    return null;
  }
  static async executeVerifySingningConfig(e, i, t) {
    const o = new java_command_builder_js_1.JavaCommandBuilder().addCalledJarFile(e).addJvmOption("verify-profile").addJvmOption("-inFile").addJvmOption(i).addJvmOption("-outFile").addJvmOption(t).build();
    await new process_utils_js_1.ProcessUtils().execute(o);
  }
  static async executeVerifySingningConfigRequest(e, i, t) {
    await (0, sign_request_js_1.sendVerifyProfileRequest)(e, {
      inFile: i,
      outFile: t
    });
  }
  static getBundleNameFromHap(e, i) {
    var t;
    const o = (0, wdk_1.cloneDeep)(null === (t = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === t ? undefined : t.appOpt);
    if (null == o ? undefined : o.bundleName) return o.bundleName;
    const r = e.bundleName;
    return r || i.getDefaultBundleName();
  }
  static searchDuplicateName(e) {
    return (0, array_util_js_1.findDuplicateElement)(e.map(e => e.name));
  }
  static checkUniqueModuleName(e) {
    const i = new Map(),
      t = new Set();
    for (const o of e) {
      const e = o.name;
      i.has(e.toLowerCase()) ? (t.add(e), t.add(i.get(e.toLowerCase()))) : i.set(e.toLowerCase(), e);
    }
    return Array.from(t);
  }
  static validateDuplicatedName(e, i, t) {
    if (undefined === e) return;
    const o = "modules" === i ? ValidateUtil.checkUniqueModuleName(e) : ValidateUtil.searchDuplicateName(e);
    0 !== o.length && ValidateUtil._log.printErrorExit("UNEQUAL_NAMES", [i, o.join(",")], [[i, t]]);
  }
  static validateContainsDefault(e, i) {
    e && e.some(e => "default" === e.name) || ValidateUtil._log.printErrorExit("NO_DEFAULT_PRODUCT", [], [[i]]);
  }
  static isDeviceTypeExist(e, i) {
    return e.some(e => i.has(e));
  }
  static isAllLiteDeviceType(e, i) {
    return e.every(e => i.has(e));
  }
  static validateHybridDeviceTypes(e) {
    ValidateUtil.isDeviceTypeExist(e, ValidateUtil.richDeviceTypes) && ValidateUtil.isDeviceTypeExist(e, ValidateUtil.liteDeviceTypes) ? ValidateUtil._log.printErrorExit("INCONSISTENT_DEVICE_TYPES") : ValidateUtil.isAllLiteDeviceType(e, ValidateUtil.liteDeviceTypes) && e.length > 1 && ValidateUtil._log.printErrorExit("UNEQUAL_MINI_SYSTEM_DEVICE_TYPES");
  }
  static validateFALiteDevice(e) {
    e.some(e => ValidateUtil.liteDeviceTypes.has(e)) && ValidateUtil._log.printErrorExit("FA_MINI_SYSTEM_DEVICE_TYPES");
  }
  static validateTargetCustomizeResources(e, i) {
    e && null !== e.match("^./?[\\s\\S]*[^ ]$") && fs.existsSync(i) || ValidateUtil._log.printErrorExit("INVALID_TARGETS_RESOURCE", [e]);
  }
  static isDefaultCheck(e, i) {
    if (0 === e.length) return;
    let t = 0;
    for (const i of e) {
      true === i.isDefault && (t += 1);
    }
    t > 1 && ValidateUtil._log.printErrorExit("ONLY_DEFAULT_CARD", [i]);
  }
  static checkUpdateEnable(e, i) {
    for (const t of e) t.updateEnabled && undefined === t.scheduledUpdateTime && undefined === t.updateDuration && ValidateUtil._log.printErrorExit("WHEN_UPDATEENABLED_EXISTS_UPDATEDURATION_AND_SCHEDULEUPDATETIME_ARE_NOT_EMPTY", [i]);
  }
  static validateLocalDependency(e, i, t) {
    var o;
    if (!(null == t ? undefined : t.length)) return;
    if (!(null === (o = null == i ? undefined : i.strictMode) || undefined === o ? undefined : o.harLocalDependencyCheck)) return;
    const r = [dependency_interface_1.DependencyType.DEPENDENCY_TYPE_HAR, dependency_interface_1.DependencyType.DEPENDENCY_TYPE_HSP],
      n = e.getProjectDir(),
      s = e.isHspModule();
    t.forEach(i => {
      var t;
      const o = i.getDependencyType(),
        a = i.getDependedVersion(),
        l = i.getDependencyEnum() === dependency_interface_1.DependencyEnum.DEV_DEPENDENCIES,
        _ = r.includes(o),
        c = s && o === dependency_interface_1.DependencyType.DEPENDENCY_TYPE_HAR;
      if (!_ || l || !(0, npm_utils_1.isLocalDependency)(a) || c) return void this._log.debug("Exit dependency chick: ", `Dependency Type: ${o};`, `Dependency value: ${a}`);
      const d = (null === (t = this.FILE_START_EXP.exec(a)) || undefined === t ? undefined : t[1]) || a,
        u = path_1.resolve(n, d);
      !path_1.isAbsolute(d) && file_util_js_1.FileUtil.isSubDir(n, u) || this._log.printErrorExit("THE_LOCAL_DEPENDENCY_BELOW_IN_MODULE_%S_IS_INVALID", [e.getName(), `"${i.getDependencyName()}":"${a}"`]);
    });
  }
}
ValidateUtil._log = ohos_logger_js_1.OhosLogger.getLogger(ValidateUtil.name), ValidateUtil.richDeviceTypes = new Set(["phone", "tablet", "tv", "wearable", "car"]), ValidateUtil.liteDeviceTypes = new Set(["lite Wearable", "smart Vision", "router"]), ValidateUtil.hosApiCompatible = new Map([[9, 9], [10, 9], [11, 9], [12, 10]]), ValidateUtil.FILE_START_EXP = /^file:(.*)/i;
