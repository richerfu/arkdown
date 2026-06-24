/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/obfuscation/get-obfuscation-rules.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as dependency_interface_js_1 from "../../project/dependency/core/dependency-interface";
import * as har_target_util_js_1 from "../../utils/har-target-util";
import * as hsp_target_utils_js_1 from "../../utils/hsp-target-utils";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("obfuscation"),
  getObfuscationOptionsWithCache = async (e, t, o) => {
    var s, r, i;
    const n = e.getTargetData().getModuleModel();
    if (!t) return;
    const l = process.hrtime.bigint(),
      a = walkFromTree(e.getModuleService().getDependencyInfo().getModulePkgNode(), e);
    _log.debug(`Resolve obfuscation options tasks ${(process.hrtime.bigint() - l) / BigInt(1e6)} ms.`);
    const u = n.getModule();
    return t.ruleOptions = {
      enable: null === (s = t.ruleOptions) || undefined === s ? undefined : s.enable,
      rules: (null === (r = t.ruleOptions) || undefined === r ? undefined : r.rules) || transformRules(null === (i = t.ruleOptions) || undefined === i ? undefined : i.files, n.getProjectDir(), u.getName(), u.getBuildProfilePath())
    }, t.consumerRules = t.consumerRules || transformRules(t.consumerFiles, n.getProjectDir(), u.getName(), u.getBuildProfilePath()), {
      selfConfig: t,
      sdkApis: [],
      obfuscationCacheDir: o,
      exportRulePath: e.getTargetData().getPathInfo().getObfuscationExportRulePath(),
      dependencies: a
    };
  };
const walkFromTree = (e, t) => {
    const o = [...e.children],
      s = new Set(),
      r = {
        libraries: [],
        hars: [],
        hspLibraries: [],
        hsps: []
      },
      i = har_target_util_js_1.HarTargetUtil.calDepHarTargets(t),
      n = hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(t);
    for (; o.length > 0;) {
      const e = o.shift();
      s.has(e.modulePkgJsonPath) || (e.npm.isLocal() ? collectLocalConfigs(e, r, o, i, n) : collectRemoteFiles(e, r), s.add(e.modulePkgJsonPath));
    }
    return r;
  },
  collectLocalConfigs = (e, t, o, s, r) => {
    var i, n, l, a;
    if (!e.module) return;
    const u = e.module.getDependencyType();
    if (u !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR && u !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP) return;
    const c = u === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR,
      d = c ? "libraries" : "hspLibraries",
      p = c ? s.get(e.moduleName) : null === (i = r.get(e.moduleName)) || undefined === i ? undefined : i.getTargetName(),
      g = null === (n = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(e.module.getModuleName(), null != p ? p : "default").arkOptions) || undefined === n ? undefined : n.obfuscation;
    if (g) {
      const o = e.module.getModule(),
        s = e.module.getDependencyRootPath(),
        r = {
          ruleOptions: {
            enable: null === (l = g.ruleOptions) || undefined === l ? undefined : l.enable,
            rules: transformRules(null === (a = g.ruleOptions) || undefined === a ? undefined : a.files, s, o.getName(), o.getBuildProfilePath())
          },
          consumerRules: transformRules(g.consumerFiles, s, o.getName(), o.getBuildProfilePath()),
          libDir: s
        };
      t[d].push(r);
    }
    c && o.push(...e.children);
  },
  collectRemoteFiles = (e, t) => {
    const o = path_1.resolve(e.npm.getDependencyRootPath(), "obfuscation.txt");
    fs_1.existsSync(o) && ("har" === e.npm.getDependencyType() && t.hars.push(o), "hsp" === e.npm.getDependencyType() && t.hsps.push(o));
  },
  transformRules = (e, t, o, s) => {
    if (!e) return [];
    const r = [];
    return "string" == typeof e ? (checkObFile(e, t, r, o, s), r) : (e.forEach(e => checkObFile(e, t, r, o, s)), r);
  },
  checkObFile = (e, t, o, s, r) => {
    const i = path_1.resolve(t, e);
    fs_1.existsSync(i) || _log.printErrorExit("OBFUSCATION_FILE_LOST", [e, i, s, r], [[e, i]]), o.push(i);
  };
export { getObfuscationOptionsWithCache };
