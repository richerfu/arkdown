/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/util.js
 */
import path_1 from "path";
import * as worker_threads_1 from "worker_threads";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as log4js_1 from "log4js";
import * as common_const_js_1 from "../../const/common-const";
import * as ability_type_enum_js_1 from "../../enum/ability-type-enum";
export const generateLoaderEnv = e => {
  var o;
  const t = {
    runtimeOS: e.runtimeOS,
    sdkInfo: e.sdkInfo,
    watchMode: e.watchMode,
    isPreview: e.isPreview ? "true" : undefined,
    aceManifestPath: e.aceManifestPath,
    appResource: e.appResource,
    rawFileResource: e.rawFileResource,
    aceModuleBuild: e.aceModuleBuild,
    aceModuleRoot: e.aceModuleRoot,
    cachePath: e.cachePath,
    aceProfilePath: e.aceProfilePath,
    aceModuleJsonPath: e.aceModuleJsonPath,
    aceSuperVisualPath: e.aceSuperVisualPath,
    aceBuildJson: e.aceBuildJson,
    compileTool: common_const_js_1.ArkPackConst.COMPILE_TOOL,
    abilityType: e.abilityType,
    aceCompileMode: e.abilityType === ability_type_enum_js_1.AbilityTypeEnum.WORKER ? ability_type_enum_js_1.AbilityTypeEnum.WORKER : e.aceCompileMode,
    externalApiPaths: e.externalApiPaths.join(path_1.delimiter),
    enableDebugLine: e.debugLine,
    isFaMode: e.isFaMode ? "true" : undefined,
    isFirstBuild: (null === (o = null == e ? undefined : e.collectImportersConfig) || undefined === o ? undefined : o.isReload) ? "false" : "true",
    widgetCompile: e.widgetCompile
  };
  process.env = {
    ...process.env,
    ...t
  }, process.env.path = process.env.Path;
};
export function setLogLevel(e) {
  var o, t;
  (0, hvigor_1.setCategoriesLevel)(null !== (o = hvigor_1.levelMap.get(e)) && undefined !== o ? o : log4js_1.levels.INFO), hvigor_logger_1.HvigorLoggerConfig.setCategoriesLevel(null !== (t = hvigor_1.levelMap.get(e)) && undefined !== t ? t : log4js_1.levels.INFO), log4js_1.default.shutdown(), log4js_1.default.configure((0, hvigor_1.getConfiguration)());
}
export function onTerminateWorker() {
  let e = {};
  return worker_threads_1.parentPort.on("message", async o => {
    (null == o ? undefined : o.type) === hvigor_1.WatchEvent.TERMINATE_WORKER && (await (0, hvigor_arkts_compose_1.writeWatchCache)(e), worker_threads_1.parentPort.postMessage(o));
  }), {
    updatePkgName2SourceRoots(o) {
      e = o;
    }
  };
}
export function onCustomWorkerRequest(e) {
  worker_threads_1.parentPort && worker_threads_1.parentPort.postMessage({
    ...e,
    event: hvigor_1.WorkerEvent.CUSTOM_WORKER_REQUEST
  });
}
