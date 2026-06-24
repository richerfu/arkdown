/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/daemon-info-service.js
 */
import * as crypto_1 from "crypto";
import process_1 from "process";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as util_js_1 from "../../../cli/wrapper/util";
import * as const_js_1 from "../../../common/const/const";
import * as hvigor_daemon_log_js_1 from "../log/hvigor-daemon-log";
import * as session_id_js_1 from "../session/session-id";
import * as daemon_info_js_1 from "./daemon-info";
import * as path_util_js_1 from "../../../common/util/path-util";
const _log = hvigor_daemon_log_js_1.HvigorDaemonLogger.getLogger("daemon");
class DaemonInfoService {
  createDefaultDaemonInfo(e, o) {
    var r;
    _log.debug(`hvigor daemon: Create default daemon info. execArgv=${[...process_1.execArgv]}`);
    const s = (0, util_js_1.getCurrentProjectCacheDirHash)();
    _log.debug(`hvigor work dir: ${s}`);
    const t = require.resolve(const_js_1.HVIGOR_ENGINE_PACKAGE_NAME);
    return {
      pid: process_1.pid,
      state: daemon_info_js_1.DaemonState.IDLE,
      address: e.address,
      cwdPath: process_1.cwd(),
      cachePath: path_util_js_1.PathUtil.getHvigorCacheDir(),
      hvigorVersion: const_js_1.CUR_HVIGOR_VERSION,
      keyId: (0, crypto_1.randomBytes)(20).toString("hex"),
      nodeParams: (0, hvigor_common_1.hash)(`${t}${s}${process_1.execArgv.toString()}`),
      nodeVersion: process_1.version,
      port: e.port,
      lastUsedTime: Date.now(),
      createdBy: null !== (r = process_1.env.CREATED_BY) && undefined !== r ? r : "command",
      sessionId: session_id_js_1.SessionIdHelper.encryptPwdByDefaultMaterial(o, _log)
    };
  }
}
export const defaultDaemonInfoService = new DaemonInfoService();
