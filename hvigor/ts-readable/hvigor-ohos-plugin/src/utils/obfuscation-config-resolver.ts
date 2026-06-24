/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/obfuscation-config-resolver.js
 */
import fs_1 from "fs";
import promises_1 from "fs/promises";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as build_option_path_info_js_1 from "../common/build-option-path-info";
import * as common_const_js_1 from "../const/common-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as build_mode_manager_js_1 from "../project/build-option/build-mode-manager";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as file_util_js_1 from "./file-util";
import * as ohos_logger_js_1 from "./log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("obfuscation"),
  resolveObfuscationOptions = async (e, o, t) => {
    var s, n, r;
    const i = e.getTargetData().getModuleModel(),
      l = e.getTargetData().getTargetName();
    if (!o) return;
    const a = await (0, resolveObfuscationDependencies)(i, e.getModuleService());
    return o.ruleOptions = {
      enable: null === (s = o.ruleOptions) || undefined === s ? undefined : s.enable,
      rules: (null === (n = o.ruleOptions) || undefined === n ? undefined : n.rules) || (0, transformRules)(null === (r = o.ruleOptions) || undefined === r ? undefined : r.files, i.getProjectDir(), i, l)
    }, o.consumerRules = o.consumerRules || (0, transformRules)(o.consumerFiles, i.getProjectDir(), i, l), {
      selfConfig: o,
      sdkApis: [],
      obfuscationCacheDir: t,
      exportRulePath: e.getTargetData().getPathInfo().getObfuscationExportRulePath(),
      dependencies: a
    };
  };
export const resolveObfuscationDependencies = async (e, o) => {
  const t = {
    libraries: [],
    hspLibraries: [],
    hars: [],
    hsps: []
  };
  return await resolveDepObfuscationOptions(e, o, t), t;
};
function handleNoneLocalModule(e, o, t, s) {
  const n = path_1.resolve(e, common_const_js_1.CommonConst.OBFUSCATION_TXT),
    r = o.getAllDependencyType().get(t);
  fs_1.existsSync(n) && (_log.debug(`Collect obfuscation config from dependency ${t}.`), r === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR && -1 === s.hars.indexOf(n) && s.hars.push(n), r === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP && -1 === s.hsps.indexOf(n) && s.hsps.push(n));
}
function handleByDepModuleType(e, o, t, s, n) {
  var r, i;
  if (e) {
    _log.debug(`Collect obfuscation config from library ${o.getName()}.`);
    const l = {
      ruleOptions: {
        enable: null === (r = e.ruleOptions) || undefined === r ? undefined : r.enable,
        rules: (0, transformRules)(null === (i = e.ruleOptions) || undefined === i ? undefined : i.files, t, o, "default")
      },
      consumerRules: (0, transformRules)(e.consumerFiles, t, o, "default"),
      libDir: t
    };
    s === module_type_enum_js_1.ModuleType.Har && n.libraries.push(l), s === module_type_enum_js_1.ModuleType.Shared && n.hspLibraries.push(l);
  }
}
const resolveDepObfuscationOptions = async (e, o, t) => {
    var s;
    const n = e.getParentProject().isOhpmProject(),
      r = ["version", "devDependencies", "dynamicDependencies", "dependencies"],
      i = n ? e.getOhPackageJson5Path() : e.getPackageJsonPath();
    if (!fs_1.existsSync(i)) return;
    const l = await hvigor_1.Json5Reader.readJson5File(i);
    if (n) {
      const o = e.getParentProject().getParameterFileObj(),
        t = {
          parameterFilePath: e.getParentProject().getParameterFileAbsolutePath(),
          ohPackageFilePath: i
        };
      file_util_js_1.FileUtil.resolveJsonObjWithoutParam(l, o, t, r);
    }
    if (!(null == l ? undefined : l.name)) return;
    const a = n ? common_const_js_1.CommonConst.OH_MODULES : common_const_js_1.CommonNodeConst.NODE_MODULES;
    for (const n in l.dependencies) {
      if (!Object.prototype.hasOwnProperty.call(null == l ? undefined : l.dependencies, n)) continue;
      const r = path_1.resolve(e.getProjectDir(), a, ...n.split("/"));
      if (!fs_1.existsSync(r)) {
        _log.warn(`Dependency ${n} not found.`);
        continue;
      }
      let i;
      try {
        i = await promises_1.lstat(r);
      } catch (e) {
        _log.warn(`Dependency ${n} not found or symbolic link is invalid.`);
        continue;
      }
      const u = i.isSymbolicLink() ? await promises_1.realpath(r) : r,
        c = resolveModuleWithPath(u, e.getAllModules());
      if (!c) {
        handleNoneLocalModule(u, o, n, t);
        continue;
      }
      const p = c.getModuleType(),
        d = null === (s = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(c.getName(), "default").arkOptions) || undefined === s ? undefined : s.obfuscation,
        _ = t.libraries.find(e => e.libDir === u) || t.hspLibraries.find(e => e.libDir === u);
      d && _ || (handleByDepModuleType(d, c, u, p, t), c.isHarModule() && (await resolveDepObfuscationOptions(c, o, t)));
    }
  },
  resolveModuleWithPath = (e, o) => {
    for (const t of o.values()) {
      if ("" === path_1.relative(t.getProjectDir(), e)) return t;
    }
  },
  transformRules = (e, o, t, s) => {
    if (!e) return [];
    const n = [];
    return "string" == typeof e ? (checkObFile(e, o, n, t, s), n) : (e.forEach(e => checkObFile(e, o, n, t, s)), n);
  };
const checkObFile = (e, o, t, s, n) => {
  const r = path_1.resolve(o, e),
    i = build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(s, n, "files");
  fs_1.existsSync(r) || _log.printErrorExit("OBFUSCATION_FILE_LOST", [e, r, s.getName(), i], [[e, r]]), t.push(r);
};
export { resolveObfuscationOptions, transformRules };
