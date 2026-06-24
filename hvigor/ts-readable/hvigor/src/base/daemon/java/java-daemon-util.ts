/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/java/java-daemon-util.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as path_const_js_1 from "../../../common/const/path-const";
import * as hvigor_user_home_js_1 from "../../../common/util/hvigor-user-home";
const JAVA_DAEMON_CACHE_FILE = "java-daemon.json";
export function writeJavaDaemonInfo2CacheFile(e) {
  const t = path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, "daemon", "cache");
  fs_extra_1.ensureDirSync(t);
  const a = path_1.join(t, "java-daemon.json");
  fs_extra_1.writeFileSync(a, JSON.stringify(e));
}
export function readJavaDaemonInfoFromCacheFile() {
  const e = path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, "daemon", "cache", "java-daemon.json");
  if (fs_extra_1.existsSync(e)) try {
    const t = fs_extra_1.readFileSync(e).toString();
    return JSON.parse(t);
  } catch (e) {
    return;
  }
}
export function writeSecretKeyFile(e) {
  const t = path_1.join((0, hvigor_user_home_js_1.getHvigorUserHomeCacheDir)(), "daemon", "cache");
  fs_extra_1.ensureDirSync(t);
  const a = path_1.join(t, "key.txt");
  fs_extra_1.writeFileSync(a, e);
}
