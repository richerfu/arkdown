/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/boot/hooks/init-env-config-props.js
 */
import fs_1 from "fs";
import path_1 from "path";
import process_1 from "process";
import * as common_const_js_1 from "../../common/options/common-const";
import * as global_data_js_1 from "../../internal/data/global-data";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
const configFileName = common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE_NAME,
  propertiesAlias = "prop",
  modeAlias = "mode",
  _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-InitEnvConfigProps");
export function initEnvConfigProps() {
  const o = global_data_js_1.globalData.cliOpts,
    t = global_data_js_1.globalData.cliEnv,
    _ = path_1.resolve(process_1.cwd(), common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE);
  fs_1.existsSync(_) || _log.printErrorExit("CANNOT_FIND_PROJECT_BUILD_FILE", [common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE, t.cwd]);
  const e = new Map();
  undefined !== o.prop && [o.prop].flat(2).forEach(o => {
    const t = o.split("=");
    e.set(t[0], t[1]);
  }), t.configProps = new Map([[configFileName, _], ["prop", e], ["mode", o.mode]]);
}
