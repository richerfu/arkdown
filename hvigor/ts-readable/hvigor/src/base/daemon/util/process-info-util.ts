/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/util/process-info-util.js
 */
import * as child_process_1 from "child_process";
import os_1 from "os";
import path_1 from "path";
import process_1 from "process";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as index_js_1 from "../../../common/util/iconv/index";
import * as exit_js_1 from "../../boot/hooks/exit";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as daemon_const_js_1 from "../const/daemon-const";
import * as daemon_info_js_1 from "../registry/daemon-info";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("process-info-util");
export function isHvigorDaemonProcess(e) {
  if (e <= 0) return false;
  try {
    process_1.kill(e, 0);
  } catch (e) {
    return false;
  }
  if ((0, hvigor_common_1.isWindows)()) {
    if (examineHvigorDaemonProcess(`${path_1.resolve("C:\\Windows", "System32", "wbem", "wmic")} process where processid="${e}" get processid,caption,commandline`, e)) return true;
    return !!examineHvigorDaemonProcess(`powershell "Get-CimInstance -ClassName Win32_Process -Filter 'ProcessId LIKE ${e}' -Property ProcessId,Caption,commandline"${os_1.EOL}`, e) || (_log.debug(`hvigor client: Check process info failed. If process with pid=${e} exists and it is an hvigor process, please check the WMI function of your environment or starting hvigor with --no-daemon param.`), false);
  }
  if ((0, hvigor_common_1.isLinux)() || (0, hvigor_common_1.isMac)()) {
    return examineHvigorDaemonProcess(`ps -fp ${e}`, e);
  }
  return false;
}
export function isDaemonProcessAlive(e) {
  try {
    return process_1.kill(e.pid, 0), true;
  } catch (e) {
    return false;
  }
}
function examineHvigorDaemonProcess(e, o) {
  const s = (0, hvigor_common_1.isWindows)() ? "GBK" : "utf-8";
  try {
    const r = (0, child_process_1.execSync)(e, {
        windowsHide: true,
        encoding: "buffer",
        stdio: "pipe"
      }),
      i = index_js_1.iconv.decode(r, s);
    return i.includes(o.toString()) && i.includes("node") && i.includes("hvigor") && i.includes("daemon-process-boot-script");
  } catch (e) {
    if (!e.stderr) return _log.warn(e), false;
    const o = index_js_1.iconv.decode(e.stderr, s);
    return _log.warn(o), false;
  }
}
export function stopDaemons(e) {
  const o = fs_extra_1.readJsonSync(daemon_const_js_1.DEFAULT_DAEMON_REGISTRY_PATH, {
    encoding: "utf-8"
  });
  let s = 0;
  Object.values(o).forEach(r => {
    r.state !== daemon_info_js_1.DaemonState.STOPPED && r.state !== daemon_info_js_1.DaemonState.BROKEN && (e || path_1.normalize(r.cwdPath) === path_1.normalize(process_1.cwd())) && (r.state = daemon_info_js_1.DaemonState.STOPPED, killDaemonProcess(r) ? s++ : delete o[r.keyId]);
  }), _log.info(`Stopped ${s} daemon${s > 1 ? "s" : ""} in total.`), fs_extra_1.writeJsonSync(daemon_const_js_1.DEFAULT_DAEMON_REGISTRY_PATH, o), (0, exit_js_1.exit)(0);
}
function killDaemonProcess(e) {
  return isHvigorDaemonProcess(e.pid) ? (process_1.kill(e.pid, "SIGTERM"), _log.info(`Stopped daemon pid ${e.pid}.`), true) : (_log.warn(`process pid ${e.pid} is not a hvigor daemon. The process will not be killed and the info will be clear. `), false);
}
export function logDaemonInfo() {
  _log.info("PID\tSTATUS\tPORT\tROOT_PATH");
  try {
    const e = fs_extra_1.readJsonSync(daemon_const_js_1.DEFAULT_DAEMON_REGISTRY_PATH, {
      encoding: "utf-8"
    });
    Object.values(e).forEach(e => {
      _log.info(`${e.pid}\t${e.state}\t${e.port}\t${e.cwdPath}`);
    });
  } finally {
    (0, exit_js_1.exit)(0);
  }
}
