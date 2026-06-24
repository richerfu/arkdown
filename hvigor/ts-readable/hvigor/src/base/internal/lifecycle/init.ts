/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/init.js
 */
import cluster_1 from "cluster";
import fs_1 from "fs";
import path_1 from "path";
import process_1 from "process";
import * as util from "util";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hook_const_js_1 from "../../../common/const/hook-const";
import * as node_config_path_info_js_1 from "../../../common/node-config-path-info";
import * as common_const_js_1 from "../../common/options/common-const";
import * as watch_config_file_js_1 from "../../daemon/cluster/watch-config-file";
import * as hvigor_config_js_1 from "../../external/core/hvigor-config";
import * as hvigor_core_js_1 from "../../external/core/hvigor-core";
import * as hvigor_js_1 from "../../external/core/hvigor";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as file_util_js_1 from "../../util/file-util";
import * as time_util_js_1 from "../../util/time-util";
import * as validate_util_js_1 from "../../util/validate-util";
import * as project_cache_service_js_1 from "../cache/project-cache-service";
import * as global_core_parameters_js_1 from "../data/global-core-parameters";
import * as global_data_js_1 from "../data/global-data";
import * as ts_check_js_1 from "../util/ts_check";
import * as hvigor_lifecycle_hook_js_1 from "./hook/hvigor-lifecycle-hook";
import * as module_impl_js_1 from "./hvigor-node-impl/module-impl";
import * as project_impl_js_1 from "./hvigor-node-impl/project-impl";
const propertiesAlias = "prop",
  log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-init");
export async function init(e) {
  var o;
  const r = global_data_js_1.globalData.cliEnv,
    t = global_data_js_1.globalData.cliOpts._,
    i = hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance();
  promptUpdateNode(), hvigor_core_js_1.hvigorCore.setExtraConfig(r.configProps.get("prop")), hvigor_core_js_1.hvigorCore.setCommandEntryTasks(t);
  const _ = r.cwd,
    a = new project_impl_js_1.ProjectImpl(path_1.basename(_), _);
  node_config_path_info_js_1.nodeConfigPathInfo.initNodeConfigPath(a), i.clear(), (0, hvigor_config_js_1.hvigorConfigInit)(a), evaluateHvigorConfig(), await i.runHook(hook_const_js_1.HookType.configEvaluated, hvigor_js_1.hvigor.getHvigorConfigInternal(), e), null === (o = hvigor_js_1.hvigor.getHvigorConfigInternal().getRootNodeDescriptorInternal().getChildNodeInternal()) || undefined === o || o.forEach(e => {
    validate_util_js_1.ValidateUtil.validateModule(new class {
      constructor() {
        this.name = e.name, this.srcPath = e.srcPath;
      }
    }());
    const o = new module_impl_js_1.ModuleImpl(a, e.name, path_1.resolve(_, e.srcPath));
    (0, file_util_js_1.exitIfNotExists)(log, o.getNodeDir(), "modules", path_1.resolve(a.getNodeDir(), common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE)), e.extraOptions.forEach((e, r) => {
      o.addExtraOption(r, e);
    }), a.addSubModule(o);
  }), hvigor_core_js_1.hvigorCore.initRootProject(a), await i.runHook(hook_const_js_1.HookType.nodesInitialized, hvigor_js_1.hvigor, e), log.debug(`Hvigor init with startParameters:${util.inspect(global_core_parameters_js_1.coreParameter.startParams)}`), global_core_parameters_js_1.coreParameter.startParams.incrementalExecution && initCacheService(a);
}
function initCacheService(e) {
  const o = process_1.hrtime();
  project_cache_service_js_1.ProjectCacheService.getInstance(e).initialize();
  const r = process_1.hrtime(o),
    t = (0, time_util_js_1.formatTime)(r);
  log.debug(`Cache service initialization finished in ${t}`);
}
function promptUpdateNode() {
  (global_core_parameters_js_1.coreParameter.startParams.analyze === global_core_parameters_js_1.AnalyzeMode.ADVANCED || global_core_parameters_js_1.coreParameter.startParams.analyze === global_core_parameters_js_1.AnalyzeMode.TRACE) && !(0, hvigor_common_1.isWindows)() && process_1.version < "v18.14.1" && log.warn("In advanced mode, to enhance performance, node version v18.14.1 or later is preferred.");
}
function evaluateHvigorConfig() {
  const e = path_1.resolve(hvigor_js_1.hvigor.getHvigorConfigInternal().getRootNodeDescriptorInternal().srcPath, common_const_js_1.HvigorCommonConst.HVIGOR_CONFIG_FILE_NAME);
  let o;
  for (const r of common_const_js_1.HvigorCommonConst.HVIGOR_CONFIG_FILE_NAME_SUFFIX) if (o = `${e}${r}`, fs_1.existsSync(o)) {
    global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck && o.endsWith(".ts") && (0, ts_check_js_1.setHvigorConfigFileTsRoot)(o), delete require.cache[require.resolve(o)];
    try {
      require(o), log.debug("hvigorconfig, required");
    } catch (e) {
      log.errorMessageExit(e.name, e.message);
    }
    cluster_1.isWorker && (0, watch_config_file_js_1.addModuleDependency)(o);
  }
}
