/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/cmake/cmake-util.js
 */
import os_1 from "os";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as build_option_path_info_js_1 from "../../common/build-option-path-info";
import * as build_mode_const_js_1 from "../../const/build-mode-const";
import * as common_const_js_1 from "../../const/common-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as generate_codemodel_js_1 from "../../model/cxx/generate-codemodel";
import * as generate_native_library_js_1 from "../../model/cxx/generate-native-library";
import * as sdk_version_js_1 from "../../version/sdk-version";
import * as file_util_js_1 from "../file-util";
import * as ohos_logger_js_1 from "../log/ohos-logger";
export class CmakeUtil {
  static parseLibraries(e, t, r) {
    var i;
    const a = new generate_codemodel_js_1.GenerateCodemodel(e),
      o = a.getCodemodel(),
      s = a.getObjFromIndex("cmakeFiles"),
      _ = new Map();
    return null === (i = null == o ? undefined : o.configurations) || undefined === i || i.flatMap(e => e.targets).forEach(e => {
      const i = generate_native_library_js_1.GenerateNativeLibrary.parseLibrary(r, e, t, s);
      _.set(e.name, i);
    }), _;
  }
  static checkNativeHeader(e, t, r) {
    fs_extra_1.existsSync(e) || CmakeUtil._log.printErrorExit("THE_PROPERTY_OF_HEADERPATH_SPECIFIES_DIRECTORY_%S_WHICH_DOES_NOT_EXIST", [e, t.getProfilePath()]), fs_extra_1.statSync(e).isFile() && CmakeUtil._log.printErrorExit("THE_PROPERTY_OF_HEADERPATH_SPECIFIES_DIRECTORY_%S_IS_NOT_DITECTORY", [e, t.getProfilePath()], [[t.getProfilePath()]]);
    fs_extra_1.readdirSync(e).forEach(t => {
      r.get(t) && CmakeUtil._log.printErrorExit("THE_FOLLOWING_PATH_CONTAINS_A_FILE_WITH_THE_SAME_NAME", [t, e, r.get(t)]), r.set(t, e);
    });
  }
  static cleanCache(e) {
    const t = path_1.resolve(e, "CMakeCache.txt");
    file_util_js_1.FileUtil.deleteFile(t);
  }
  static getCmakeListDir(e, t, r) {
    const i = e.getProjectDir();
    if (!r) return path_1.resolve(i, "./src/main/cpp");
    let a = r.path;
    const o = build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(e, t, "path");
    undefined === a && (a = "./src/main/cpp/CMakeLists.txt"), "" === a.trim() && this._log.printErrorExit("THE_CONFIGURED_CMAKE_SCRIPT_IN_EXTERNALNATIVEOPTIONS_PATH_CAN_NOT_BE_EMPTY", [o]);
    const s = path_1.resolve(i, a);
    return fs_extra_1.existsSync(s) || this._log.printErrorExit("THE_CONFIGURED_CMAKE_SCRIPT_IN_EXTERNALNATIVEOPTIONS_PATH_DOES_NOT_EXIST", [o]), "CMakeLists.txt" !== path_1.basename(s) && this._log.printErrorExit("THE_CONFIGURED_CMAKE_SCRIPT_NAME_%S_SHOULD_BE_CMAKELISTSTXT", [path_1.basename(s), o]), path_1.resolve(i, a, "..");
  }
  static resolveArch(e, t, r) {
    if (e.indexOf(this.ARCH_X86_64) > -1) return;
    const i = t ? this.OHOS_MIN_NDK_FOR_X86_64 : this.HOS_MIN_NDK_FOR_X86_64;
    (r.equals(i) || r.isHigherThan(i)) && e.push(this.ARCH_X86_64);
  }
  static getMayBeOccurErrorsFilePath(e) {
    return [e.getProfilePath(), " If there are no issues in this file, perhaps you have used 'overrides' in 'hvigorfile.ts', please check it."].join(`${os_1.EOL}\t`);
  }
  static checkAbiFilters(e, t, r, i) {
    return t && (null == e ? undefined : e.includes("armeabi-v7a")) ? this._log.printErrorExit("ARMEABIV7A_NOT_SUPPORTED_FORHARMONYOS", [build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(r, i, "abiFilters")]) : t || 1 !== (null == e ? undefined : e.length) || "armeabi-v7a" !== e[0] || this._log.printErrorExit("ARMEABIV7A_NOT_SUPPORTED_OPENHARMONY_AS_THE_ONLY_OPTION", [build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(r, i, "abiFilters")]), e && e.length > 0 ? e : ["arm64-v8a"];
  }
  static nativeTaskCondition(e, t, r) {
    if (undefined === r) return false;
    const i = t.getTargetName();
    if (e.isHarModule() && i === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) {
      const t = e.getSourceSetByTargetName(common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET).getCodeMap().has(code_type_enum_js_1.CodeType.CPP),
        r = e.getSourceSetByTargetName(common_const_js_1.DefaultTargetConst.DEFAULT_TARGET).getCodeMap().has(code_type_enum_js_1.CodeType.CPP);
      if (!t && r) return true;
    }
    return e.getSourceSetByTargetName(i).getCodeMap().has(code_type_enum_js_1.CodeType.CPP);
  }
  static mergeCommandLine(e, t) {
    const r = CmakeUtil.parseCmakeArgs(t, undefined);
    return CmakeUtil.parseCmakeArgs(e, r), CmakeUtil.toCommandLine(r, e);
  }
  static toCommandLine(e, t) {
    const r = [t[0]];
    return e.forEach(e => r.push(CmakeUtil.assembleArg(e))), r;
  }
  static parseCmakeArgs(e, t) {
    return t = t || new Map(), e.forEach(e => {
      const r = CmakeUtil.parseArg(e.trim());
      if (undefined !== r) {
        const e = r.getPrefix() + r.getKey();
        t.has(e) || t.set(e, r);
      }
    }), t;
  }
  static parseStlFromCMakeCache(e) {
    var t;
    const r = {
        name: "OHOS_STL",
        value: "c++_shared",
        type: "STRING"
      },
      i = new generate_codemodel_js_1.GenerateCodemodel(e).getObjFromIndex("cache");
    return i && null !== (t = i.entries.find(e => "OHOS_STL" === e.name)) && undefined !== t ? t : r;
  }
  static getCMakeArguments(e) {
    return e ? "string" == typeof e ? [...e.trim().split(" ").filter(e => e.startsWith("-"))] : [...e].flatMap(e => e.trim().split(" ")).filter(e => e.startsWith("-")) : [];
  }
  static parseArg(e) {
    if (!e.startsWith("-")) return;
    const t = e.substring(0, 2),
      r = e.substring(2),
      i = r.indexOf("=");
    if (-1 === i) return new CmakeArg(t, r, undefined);
    {
      const e = r.substring(0, i).trim(),
        a = r.substring(i + 1).trim();
      return new CmakeArg(t, e, a);
    }
  }
  static getHarExtraParams(e) {
    var t, r, i;
    const a = {
      compatibleSdkVersion: e.targetData.getCompatibleApiVersion(),
      compatibleSdkType: e.targetData.isHarmonyOS() ? "HarmonyOS" : "OpenHarmony",
      obfuscated: e._harExtendInfo.isObfuscatedHar(),
      nativeComponents: []
    };
    if (!e.hasNativeOption) return delete a.nativeComponents, a;
    const o = null === (r = null === (t = e.targetService.getBuildOption()) || undefined === t ? undefined : t.nativeLib) || undefined === r ? undefined : r.librariesInfo,
      s = new Map();
    if (o && o.length > 0) for (const e of o) s.set(e.name, e.linkLibraries);
    return null === (i = CmakeUtil.getCppSoName(e)) || undefined === i || i.forEach(t => {
      var r, i;
      const o = {
        name: t,
        compatibleSdkVersion: e.targetData.getCompatibleApiVersion(),
        compatibleSdkType: e.targetData.isHarmonyOS() ? "HarmonyOS" : "OpenHarmony",
        linkLibraries: null !== (r = s.get(t)) && undefined !== r ? r : []
      };
      null === (i = a.nativeComponents) || undefined === i || i.push(o);
    }), a;
  }
  static getCppSoName(e) {
    var t;
    if (e.hasNativeOption && undefined !== e.cmakeListDir) {
      const r = new Set(),
        i = CmakeUtil.checkAbiFilters(null === (t = e.targetService.getBuildOption().externalNativeOptions) || undefined === t ? undefined : t.abiFilters, e.targetData.isHarmonyOS(), e.moduleModel, e.targetName);
      for (const t of i) {
        const i = path_1.resolve(e.pathInfo.getNinjaWorkDir(), t);
        CmakeUtil.parseLibraries(i, e.targetName, t).forEach(e => {
          e.getNameOnDisk() && r.add(e.getNameOnDisk());
        });
      }
      return r;
    }
  }
  static assembleArg(e) {
    return undefined === e.getValue() ? e.getPrefix() + e.getKey() : `${e.getPrefix() + e.getKey()}=${e.getValue()}`;
  }
  static mkCodeModelRequest(e) {
    fs_extra_1.mkdirSync(path_1.resolve(e, ".cmake", "api", "v1", "query", "codemodel-v2"), {
      recursive: true
    }), fs_extra_1.mkdirSync(path_1.resolve(e, ".cmake", "api", "v1", "query", "cmakeFiles-v1"), {
      recursive: true
    }), fs_extra_1.mkdirSync(path_1.resolve(e, ".cmake", "api", "v1", "query", "cache-v2"), {
      recursive: true
    });
  }
  static getConfigCMakeBuildType(e) {
    if (e === build_mode_const_js_1.BuildMode.DEBUG) return common_const_js_1.CMakeBuildType.Debug;
    const t = hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.HvigorConfigConst.OHOS_DEFAULTS_RELEASE_CMAKEBUILDTYPE),
      r = Object.values(common_const_js_1.CMakeBuildType);
    return t && !r.includes(t) && CmakeUtil._log.warn("The configured cmake build type for 'Release' in hvigor-config is invalid. The configuration item must be one of 'Release', 'Debug', or 'RelWithDebInfo'. Please check it."), t && r.includes(t) ? t : common_const_js_1.CMakeBuildType.Release;
  }
}
CmakeUtil._log = ohos_logger_js_1.OhosLogger.getLogger("Cmake"), CmakeUtil._nativeCacheDir = path_1.resolve(process.cwd(), ".hvigor", "cache"), CmakeUtil.ARCH_X86_64 = "x86_64", CmakeUtil.OHOS_MIN_NDK_FOR_X86_64 = new sdk_version_js_1.SdkVersion("3.2.10.6"), CmakeUtil.HOS_MIN_NDK_FOR_X86_64 = new sdk_version_js_1.SdkVersion("3.2.2.6");
export class CmakeArg {
  constructor(e, t, r) {
    this._prefix = e, this._key = t, this._value = r;
  }
  getPrefix() {
    return this._prefix;
  }
  setPrefix(e) {
    this._prefix = e;
  }
  getKey() {
    return this._key;
  }
  setKey(e) {
    this._key = e;
  }
  getValue() {
    return this._value;
  }
  setValue(e) {
    this._value = e;
  }
}
