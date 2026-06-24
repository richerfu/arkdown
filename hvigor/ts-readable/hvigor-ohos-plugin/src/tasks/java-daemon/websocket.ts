/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/java-daemon/websocket.js
 */
import node_crypto_1 from "node:crypto";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import ws_1 from "ws";
import * as ohos_logger_1 from "../../utils/log/ohos-logger";
import * as hvigor_user_home_1 from "@ohos/hvigor/src/common/util/hvigor-user-home";
const _logger = ohos_logger_1.OhosLogger.getLogger("JavaDaemonWebSocket");
export var MessageType, ErrorCode;
function createHmacSignature(e, o) {
  const r = node_crypto_1.createHmac("sha256", e, {
    encoding: "utf-8"
  });
  return r.update(o), r.digest("hex");
}
function readSecretKey() {
  const e = path_1.join((0, hvigor_user_home_1.getHvigorUserHomeCacheDir)(), "daemon", "cache", "key.txt");
  if (fs_extra_1.existsSync(e)) return fs_extra_1.readFileSync(e).toString();
}
export function sendSocketRequest(e, o) {
  return new Promise((r, t) => {
    const s = (0, hvigor_1.readJavaDaemonInfoFromCacheFile)();
    s && s.port || t({
      code: ErrorCode.NETWORK_ERROR,
      message: "read java daemon port failed"
    });
    let a = `sessionId=${s.sessionId}&jar=${o}`;
    const n = readSecretKey();
    if (n) {
      a += `&signature=${createHmacSignature(n, a)}`;
    }
    a = encodeURI(a);
    const _ = new ws_1(`ws://127.0.0.1:${s.port}?${a}`);
    _.on("error", e => {
      t({
        code: ErrorCode.NETWORK_ERROR,
        message: e.message
      });
    }), _.on("open", () => {
      _.send(JSON.stringify(e));
    }), _.on("close", (e, o) => {
      _logger.debug(`java daemon socket close code:${e} reason:${o.toString()}`), 1e3 !== e && t({
        code: ErrorCode.NETWORK_ERROR,
        message: o.toString()
      });
    }), _.on("message", e => {
      _logger.debug(`java daemon socket received message:${e.toString()}`);
      try {
        const o = JSON.parse(e.toString());
        0 === o.code ? r({
          code: ErrorCode.SUCCESS,
          message: "ok"
        }) : t({
          code: o.code,
          message: o.message
        });
      } catch (e) {
        t({
          code: ErrorCode.FAIL,
          message: e.message
        });
      }
      _.close(1e3, "close by user");
    });
  });
}
!function (e) {
  e.SIGN_APP = "SIGN_APP", e.VERIFY_PROFILE = "VERIFY_PROFILE", e.SIGN_SDK = "SIGN_SDK", e.PACK_APP = "PACK_APP";
}(MessageType = MessageType || (MessageType = {})), function (e) {
  e[e.SUCCESS = 0] = "SUCCESS", e[e.FAIL = -1] = "FAIL", e[e.NETWORK_ERROR = 1006] = "NETWORK_ERROR", e[e.CLASS_NOT_FOUND = 1007] = "CLASS_NOT_FOUND";
}(ErrorCode = ErrorCode || (ErrorCode = {}));
