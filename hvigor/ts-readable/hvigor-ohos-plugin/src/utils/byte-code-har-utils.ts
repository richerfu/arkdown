/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/byte-code-har-utils.js
 */
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as json_util_js_1 from "./json-util";
import * as ohos_logger_js_1 from "./log/ohos-logger";
import * as inject_util_js_1 from "./inject-util";
import * as code_type_enum_1 from "../enum/code-type-enum";
import * as build_directory_const_1 from "../const/build-directory-const";
import * as common_const_1 from "../const/common-const";
const logger = ohos_logger_js_1.OhosLogger.getLogger("byte-code-har-utils"),
  isHar = e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR,
  isNpmPackage = e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER,
  isByteCodeHar = e => e.isByteCodeHarDependency(),
  isBundledDependency = e => e.isBundleDependency(),
  isSourceCodeHar = e => isHar(e) && !e.isByteCodeHarDependency(),
  isBlockedByByteCodeHar = e => !!e.parent && isByteCodeHar(e.parent.npm),
  getPackageNameFromImportee = e => {
    const t = e.startsWith("@"),
      o = e.split(/[/\\]/);
    return t ? o.slice(0, 2).join("/") : o[0];
  },
  getFinalNodes = e => {
    const t = e.filter(e => !e.npm.isHspDependency() && !e.npm.isSODependency());
    return inject_util_js_1.InjectUtil.isByteCodeHarOptimize() ? t : t.filter(e => e.npm.getDependencyEnum() !== dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES);
  },
  collectByteCodeHarInfo = (e, t) => {
    const o = e.getPackageName(),
      n = e.getDependencyRootPath(),
      s = path_1.join(n, code_type_enum_1.CodeType.ETS, build_directory_const_1.BuildArtifactConst.MODULES_ABC);
    if (t.has(s)) return;
    if (!fs_extra_1.existsSync(s)) return void logger.debug(`Dependency [${o}] is byte-code HAR, but '${code_type_enum_1.CodeType.ETS}/${build_directory_const_1.BuildArtifactConst.MODULES_ABC}' does not exists.`);
    const r = path_1.join(n, code_type_enum_1.CodeType.ETS, build_directory_const_1.BuildArtifactConst.SOURCEMAPS_MAP),
      d = collectByteCodeHarInsightIntentJsonPath(e),
      c = e.getPackageJsonObj(),
      i = {
        abcPath: s,
        packageName: o,
        compatibleSdkVersion: null == c ? undefined : c.compatibleSdkVersion,
        compatibleSdkVersionStage: null == c ? undefined : c.compatibleSdkVersionStage,
        ...(fs_extra_1.existsSync(r) ? {
          sourceMapsPath: r
        } : {}),
        ...(d ? {
          insightIntentJsonPath: d
        } : {})
      };
    t.set(n, i);
  },
  collectEntryPath = (e, t) => {
    const o = e.getDependencyRootPath(),
      n = e.getDependencyMainFileRelativePath(),
      s = n ? path_1.resolve(o, n) : e.getDependencyMainFilePath();
    !t.has(s) && fs_extra_1.existsSync(s) && t.set(s, false);
  },
  collectSourceCodeHarInfo = (e, t) => {
    collectEntryPath(e, t);
    const o = e.getDependencyRootPath(),
      n = path_1.resolve(o, build_directory_const_1.BuildDirConst.SRC, build_directory_const_1.BuildDirConst.MAIN, code_type_enum_1.CodeType.ETS);
    !t.has(n) && fs_extra_1.existsSync(n) && fs_extra_1.statSync(n).isDirectory() && t.set(n, true);
  },
  collectExtraImportees = (e, t) => {
    var o;
    const n = e.getPackageJsonObj();
    ((null === (o = null == n ? undefined : n.metadata) || undefined === o ? undefined : o.extraImportees) || []).forEach(e => {
      t.add(e);
    });
  },
  collectByteCodeHarInsightIntentJsonPath = e => {
    var t, o, n;
    const s = e.getPackageJsonObj(),
      r = null === (o = null === (t = null == s ? undefined : s.metadata) || undefined === t ? undefined : t.resource) || undefined === o ? undefined : o.directories,
      d = (null == r ? undefined : r.length) ? r : ["./src/main/resources"];
    for (const t of d) {
      const o = e.getInsightIntentPathByResourceDir(path_1.resolve(e.getDependencyRootPath(), t));
      if (fs_extra_1.existsSync(o) && (null === (n = json_util_js_1.JsonUtil.getJson5Obj(o)) || undefined === n ? undefined : n.insightIntentsSrcEntry)) return o;
    }
  },
  processBundledNodeDependencies = e => {
    var t;
    const {
        node: o,
        childrenPendingOrProcessed: n
      } = e,
      s = o.npm.getDependencyRootPath();
    if (n.has(s)) return;
    n.add(s);
    const r = o.npm.getPackageJsonObj(),
      d = (null === (t = null == r ? undefined : r.metadata) || undefined === t ? undefined : t.compiledPackageNames) || [],
      c = getFinalNodes(o.children);
    for (; c.length;) {
      const t = c.shift(),
        o = t.npm.getPackageName();
      d.includes(o) ? c.push(...getFinalNodes(t.children)) : processNode({
        ...e,
        node: t,
        blockedByByteCodeHar: true
      });
    }
  },
  processBundledNode = e => {
    const {
        node: t,
        byteCodeInfo: o,
        extraImportees: n
      } = e,
      s = t.npm;
    return !!isBundledDependency(s) && (collectByteCodeHarInfo(s, o), collectExtraImportees(s, n), processBundledNodeDependencies(e), true);
  },
  enqueueChildren = e => {
    const {
        node: t,
        childrenPendingOrProcessed: o,
        queue: n
      } = e,
      s = t.npm.getDependencyRootPath();
    o.has(s) || (o.add(s), n.push(...getFinalNodes(t.children)));
  },
  processByteCodeHarNode = e => {
    const {
        node: t,
        byteCodeInfo: o,
        extraImportees: n
      } = e,
      s = t.npm;
    return !(!isByteCodeHar(s) || isBundledDependency(s)) && (collectByteCodeHarInfo(s, o), collectExtraImportees(s, n), enqueueChildren(e), true);
  },
  processSourceCodeHarNode = e => {
    const {
        node: t,
        queue: o,
        otherCompileEntrances: n,
        blockedByByteCodeHar: s
      } = e,
      r = t.npm;
    return !!isSourceCodeHar(r) && ((s || isBlockedByByteCodeHar(t)) && collectSourceCodeHarInfo(r, n), enqueueChildren(e), true);
  },
  processNpmPackageNode = e => {
    const {
        node: t,
        queue: o,
        otherCompileEntrances: n,
        blockedByByteCodeHar: s
      } = e,
      r = t.npm;
    return !!isNpmPackage(r) && ((s || isBlockedByByteCodeHar(t)) && collectEntryPath(r, n), true);
  },
  chains = [processBundledNode, processByteCodeHarNode, processSourceCodeHarNode, processNpmPackageNode],
  processNode = e => {
    chains.some(t => t(e));
  },
  processExtraImportees = (e, t, o) => {
    const n = new Map();
    e.getModuleService().getOtherDependencies().forEach(e => {
      n.set(e.getPackageName(), e);
    }), t.forEach(e => {
      const t = getPackageNameFromImportee(e);
      if (!n.has(t)) return void logger.debug(`Can not find dependency [${t}], make sure it has been installed successfully`);
      const s = n.get(t);
      if (t === e) return void collectEntryPath(s, o);
      const r = path_1.join(s.getDependencyRootPath(), e.substring(t.length)),
        d = (0, hvigor_arkts_compose_1.findSuitableFilePath)(r, hvigor_arkts_compose_1.SUPPORT_EXTENSIONS);
      d ? o.set(d, false) : logger.debug(`Can not find file path by importee: ${e} [${t}]`);
    });
  },
  collectByteCodeInfoAndOtherCompileEntrances = e => {
    const t = e.getModuleService(),
      o = inject_util_js_1.InjectUtil.isByteCodeHarOptimize() ? t.getAllDirectPkgNodes() : t.getDirectPkgNodes(),
      n = getFinalNodes(o),
      s = new Set(),
      r = new Set(),
      d = new Map(),
      c = new Map();
    for (; n.length;) {
      const e = n.shift();
      processNode({
        node: e,
        queue: n,
        byteCodeInfo: c,
        extraImportees: s,
        childrenPendingOrProcessed: r,
        otherCompileEntrances: d
      });
    }
    return processExtraImportees(e, s, d), {
      byteCodeInfo: c,
      otherCompileEntrances: d
    };
  };
export const checkByteCodeHar = ({
  logger: e,
  compileApiVersion: t,
  compatibleApiVersion: o,
  useNormalizedOHMUrl: n
}) => {
  (t < sdk_const_js_1.ApiVersion.API_VERSION_12 || o < sdk_const_js_1.ApiVersion.API_VERSION_12) && e.printErrorExit("BYTECODEHAR_NOT_SUPPORTED_WHEN_BLOW_API_VERSION_12"), n || e.printErrorExit("BYTECODEHAR_NOT_SUPPORTED_WHEN_USENORMALLIZEDOHMURL_IS_NOT_TRUE");
};
export const shouldTreatHarAsHap = e => inject_util_js_1.InjectUtil.isLocalTest() || inject_util_js_1.InjectUtil.isPreviewProcess() || inject_util_js_1.InjectUtil.isOhosTest() && e === common_const_1.DefaultTargetConst.OHOS_TEST_TARGET;
export const realGetByteCodeHarToDependencyKeys = e => {
  var t;
  const o = e.getModuleService(),
    n = inject_util_js_1.InjectUtil.isByteCodeHarOptimize() ? o.getAllDirectPkgNodes() : o.getDirectPkgNodes(),
    s = getFinalNodes(n),
    r = new Map(),
    d = new Map(),
    c = new Set(),
    i = s.map(e => ({
      node: e,
      isVisited: false
    }));
  for (; i.length > 0;) {
    const {
        node: e,
        isVisited: o
      } = i.pop(),
      n = e.npm,
      s = n.getDependencyName(),
      a = getFinalNodes(e.children);
    if (!o) {
      if (c.has(s)) {
        logger.debug(`Dependency key: '${s}' already processed, just ignore.`);
        continue;
      }
      d.set(s, []), c.add(s), i.push({
        node: e,
        isVisited: true
      });
      for (let e = a.length - 1; e >= 0; e--) i.push({
        node: a[e],
        isVisited: false
      });
      continue;
    }
    const l = d.get(s) || [];
    for (const e of a) {
      const o = e.npm.getDependencyName();
      l.push(o), (null === (t = d.get(o)) || undefined === t ? undefined : t.length) && l.push(...d.get(o));
    }
    n.isByteCodeHarDependency() && r.set(s, Array.from(new Set(l)));
  }
  return logger.debug(`ByteCodeHar2DependencyKeys in module “${o.getModuleModel().getName()}”: ${JSON.stringify(Object.fromEntries(r), null, 2)}`), r;
};
export const getByteCodeHarToDependencyKeys = e => {
  try {
    return (0, realGetByteCodeHarToDependencyKeys)(e);
  } catch (e) {
    return logger.debug(`Execute 'getByteCodeHarToDependencyKeys' failed: ${null == e ? undefined : e.message}`), new Map();
  }
};
export { collectByteCodeInfoAndOtherCompileEntrances };
