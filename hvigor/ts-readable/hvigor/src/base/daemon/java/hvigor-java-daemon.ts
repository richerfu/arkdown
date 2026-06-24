/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/java/hvigor-java-daemon.js
 */
import * as child_process_1 from "child_process";
import cluster_1 from "cluster";
import * as crypto_1 from "crypto";
import fs_1 from "fs";
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import ws_1 from "ws";
import * as index_js_1 from "../../../common/util/iconv/index";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as hvigor_config_reader_js_1 from "../../util/hvigor-config-reader";
import * as session_id_js_1 from "../session/session-id";
import * as java_daemon_util_js_1 from "./java-daemon-util";
var _a, _b;
const _log = hvigor_log_js_1.HvigorLogger.getLogger("JavaDaemon"),
  DEFAULT_START_PORT = 45050,
  charSet = (0, hvigor_common_1.isWindows)() ? "GBK" : "UTF-8",
  HVIGOR_JAVA_DAEMON_JAR = "hvigor-java-daemon.jar",
  HVIGOR_JAVA_DAEMON_JAR_PATH = path_1.resolve(__dirname, "../../../../lib", HVIGOR_JAVA_DAEMON_JAR),
  jvmXmxValue = Number(process.env.Xmx) || (null === (_b = null === (_a = hvigor_config_reader_js_1.HvigorConfigReader.getHvigorConfig()) || undefined === _a ? undefined : _a.javaOptions) || undefined === _b ? undefined : _b.Xmx) || 512;
let javaProcessId = 0;
function tryStartDaemonAtPort(e, o, r) {
  const a = (0, crypto_1.randomBytes)(32).toString("hex"),
    i = session_id_js_1.SessionIdHelper.encryptPwdByDefaultMaterial(a, _log),
    t = (0, child_process_1.spawn)("java", [`-Xmx${jvmXmxValue}m`, "-jar", e, o.toString(), i], {
      windowsHide: true
    });
  t.on("spawn", () => {
    _log.debug(`java daemon started at port ${o} pid ${t.pid}`), javaProcessId = t.pid;
    try {
      (0, java_daemon_util_js_1.writeJavaDaemonInfo2CacheFile)({
        port: o,
        pid: t.pid,
        sessionId: i
      });
      const e = (0, crypto_1.randomBytes)(32).toString("hex");
      (0, java_daemon_util_js_1.writeSecretKeyFile)(e);
    } catch (e) {
      _log.error(`java daemon writeJavaDaemonInfo2CacheFile failed ${e}`);
    }
  }), t.on("error", e => {
    _log.debug(`on error: ${e}`);
  }), t.stdout.on("data", e => {}), t.stderr.on("data", a => {
    _log.debug("javaProcess on stderr"), -1 !== a.toString().indexOf("java.net.BindException: Address already in use") && (o++, ++r < 10 && tryStartDaemonAtPort(e, o, r));
  }), t.once("exit", (e, o) => {
    _log.debug(`java daemon exit code:${e} signal:${o}`), javaProcessId = undefined;
  });
}
function tryConnect(e, o) {
  return new Promise((r, a) => {
    const i = new ws_1(`ws://127.0.0.1:${e}?sessionId=${o}`);
    i.on("error", function (e) {
      a(e);
    }), i.on("open", function () {
      i.close(), r("");
    });
  });
}
function execSystemCommand(e) {
  try {
    const o = (0, child_process_1.execSync)(e, {
      windowsHide: true,
      encoding: "buffer",
      stdio: "pipe"
    });
    return index_js_1.iconv.decode(o, charSet);
  } catch (e) {
    if (e.stderr) {
      const o = index_js_1.iconv.decode(e.stderr, charSet);
      _log.warn(o);
    } else _log.warn(e);
  }
}
function killByPid(e) {
  const o = execSystemCommand(`ps -fp ${e}`);
  if (o && o.includes("java -jar") && o.includes(HVIGOR_JAVA_DAEMON_JAR) && !o.includes(HVIGOR_JAVA_DAEMON_JAR_PATH)) try {
    process.kill(Number(e), "SIGKILL");
  } catch (e) {
    _log.warn(e);
  }
}
function killByGrep() {
  const e = execSystemCommand(`ps -ef | grep ${HVIGOR_JAVA_DAEMON_JAR}`);
  if (!e) return;
  const o = e.split("\n");
  for (const e of o) if (e.includes("java -jar") && !e.includes(HVIGOR_JAVA_DAEMON_JAR_PATH)) {
    const o = e.trim().split(/\s+/);
    if (o.length < 2) continue;
    const r = o[1];
    try {
      process.kill(Number(r), "SIGKILL");
    } catch (e) {
      _log.warn(e);
    }
  }
}
export function startJavaDaemon() {
  if (cluster_1.isPrimary) {
    if (_log.debug(`hvigor java daemon: Starting java daemon. jvmXmxValue=${jvmXmxValue}m, jarPath=${HVIGOR_JAVA_DAEMON_JAR_PATH}`), !fs_1.existsSync(HVIGOR_JAVA_DAEMON_JAR_PATH)) return void _log.error(`${HVIGOR_JAVA_DAEMON_JAR} not exist at path ${HVIGOR_JAVA_DAEMON_JAR_PATH}`);
    const e = (0, java_daemon_util_js_1.readJavaDaemonInfoFromCacheFile)();
    _log.debug(`hvigor java daemon: Read cache file, \n    port=${null == e ? undefined : e.port}, pid=${null == e ? undefined : e.pid}, sessionId exists=${!!(null == e ? undefined : e.sessionId)}`), ((0, hvigor_common_1.isLinux)() || (0, hvigor_common_1.isMac)()) && ((null == e ? undefined : e.pid) ? (_log.debug(`hvigor java daemon: Killing existing java daemon by pid=${e.pid}`), killByPid(e.pid)) : (_log.debug("hvigor java daemon: Killing existing java daemon by grep"), killByGrep())), (null == e ? undefined : e.port) && (null == e ? undefined : e.sessionId) ? tryConnect(e.port, null == e ? undefined : e.sessionId).then(() => {
      _log.debug("hvigor java daemon: tryConnect success");
    }, e => {
      _log.debug(`hvigor java daemon: tryConnect failed, ${e}, will start new daemon`), tryStartDaemonAtPort(HVIGOR_JAVA_DAEMON_JAR_PATH, 45050, 0);
    }) : (_log.debug("hvigor java daemon: No valid cache info, will start new daemon"), tryStartDaemonAtPort(HVIGOR_JAVA_DAEMON_JAR_PATH, 45050, 0));
  }
}
process.once("exit", () => {
  if (_log.debug(`master process exit pid ${process.pid}`), javaProcessId) try {
    process.kill(javaProcessId), _log.debug(`kill java process pid ${javaProcessId}`);
  } catch (e) {
    _log.error(`kill java process failed ${e}`);
  }
});
