/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/session/session-id.js
 */
import buffer_1 from "buffer";
import * as crypto_1 from "crypto";
import fs_1 from "fs";
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as global_core_parameters_js_1 from "../../internal/data/global-core-parameters";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as daemon_const_js_1 from "../const/daemon-const";
import * as hvigor_user_home_js_1 from "../../../common/util/hvigor-user-home";
const log = hvigor_log_js_1.HvigorLogger.getLogger("SessionIdHelper");
export class SessionIdHelper {
  static encryptPwdByDefaultMaterial(e, t) {
    return SessionIdHelper.encryptPwd(e, t);
  }
  static encryptPwd(e, t) {
    let r = buffer_1.Buffer.from("");
    try {
      const t = new Int8Array(buffer_1.Buffer.from(e));
      r = SessionIdHelper.encrypt(SessionIdHelper.key, t);
    } catch (e) {
      t.printErrorExit("SECTION_ID_ERROR", ["Encrypt failed"]), global_core_parameters_js_1.coreParameter.startParams.printStackTrace && e.stack && t.printErrorExit("SECTION_ID_ERROR", [e.stack]);
    }
    return r.toString("hex");
  }
  static encrypt(e, t) {
    const r = new Int8Array((0, crypto_1.randomBytes)(12)),
      n = crypto_1.default.createCipheriv("aes-128-gcm", e, r),
      o = n.update(t),
      s = n.final(),
      a = new Int8Array(n.getAuthTag()),
      _ = new Int8Array(buffer_1.Buffer.concat([o, s])),
      i = _.length + a.length,
      l = new Int8Array(4 + r.length + _.length + a.length);
    l[0] = i >> 24 & 255, l[1] = i >> 16 & 255, l[2] = i >> 8 & 255, l[3] = 255 & i;
    for (let e = 0; e < r.length; e++) l[e + 4] = r[e];
    for (let e = 0; e < _.length; e++) l[e + 16] = _[e];
    const d = 4 + r.length + _.length;
    for (let e = 0; e < a.length; e++) l[e + d] = a[e];
    return buffer_1.Buffer.from(l);
  }
  static decryptPwdByDefaultMaterial(e, t) {
    return SessionIdHelper.decryptPwd(e, t);
  }
  static decryptPwd(e, t) {
    let r = buffer_1.Buffer.from("");
    try {
      const t = new Int8Array(buffer_1.Buffer.from(e, "hex"));
      r = SessionIdHelper.decrypt(SessionIdHelper.key, t);
    } catch (e) {
      t.printErrorExit("SECTION_ID_ERROR", ["Decryption failed"]), global_core_parameters_js_1.coreParameter.startParams.printStackTrace && e.stack && t.printErrorExit("SECTION_ID_ERROR", [e.stack]);
    }
    return r.toString("utf-8");
  }
  static decrypt(e, t) {
    const r = (255 & t[0]) << 24 | (255 & t[1]) << 16 | (255 & t[2]) << 8 | 255 & t[3],
      n = t.length - 4 - r,
      o = t.slice(4, 4 + n),
      s = crypto_1.default.createDecipheriv("aes-128-gcm", e, o),
      a = t.slice(t.length - 16);
    s.setAuthTag(a);
    const _ = s.update(t.subarray(4 + n, t.length - 16)),
      i = s.final();
    return buffer_1.Buffer.concat([_, i]);
  }
  static getKey(e, t) {
    fs_1.existsSync(daemon_const_js_1.DEFAULT_DAEMON_META_PATH) || SessionIdHelper.createMaterial(t), fs_1.statSync(daemon_const_js_1.DEFAULT_DAEMON_META_PATH).isDirectory() || t.printErrorExit("SECTION_ID_ERROR", ["No materials."]);
    const r = fs_1.readdirSync(daemon_const_js_1.DEFAULT_DAEMON_META_PATH);
    0 === r.length && t.printErrorExit("SECTION_ID_ERROR", ["No materials."]), this.dirs.forEach(e => {
      r.includes(e) || t.printErrorExit("SECTION_ID_ERROR", [`Can not find signing material ${e}`]);
    });
    const n = SessionIdHelper.readFd(path_1.resolve(daemon_const_js_1.DEFAULT_DAEMON_META_PATH, this.dirs[0]), t),
      o = SessionIdHelper.readSalt(path_1.resolve(daemon_const_js_1.DEFAULT_DAEMON_META_PATH, this.dirs[1]), t),
      s = SessionIdHelper.getRootKey(n, o, t),
      a = SessionIdHelper.readWorkMaterial(path_1.resolve(daemon_const_js_1.DEFAULT_DAEMON_META_PATH, this.dirs[2]), t);
    return new Int8Array(SessionIdHelper.decrypt(s, a));
  }
  static getRootKey(e, t, r) {
    const n = e.concat(SessionIdHelper.component),
      o = SessionIdHelper.xorComponents(n, r),
      s = crypto_1.default.pbkdf2Sync(o.toString(), t, 1e4, 16, "sha256");
    return new Int8Array(s);
  }
  static xorComponents(e, t) {
    e.forEach(e => {
      16 !== e.length && t.printErrorExit("SECTION_ID_ERROR", ["Signing material data error!"]);
    });
    let r = SessionIdHelper.xor(e[0], e[1], t);
    for (let n = 2; n < e.length; n++) r = SessionIdHelper.xor(r, e[n], t);
    return buffer_1.Buffer.from(r);
  }
  static xor(e, t, r) {
    e.byteLength !== t.byteLength && r.printErrorExit("SECTION_ID_ERROR", ["Signing material data error!"]);
    const n = new Int8Array(e.byteLength);
    for (let r = 0; r < e.byteLength; r++) n[r] = e[r] ^ t[r];
    return n;
  }
  static readFd(e, t) {
    fs_1.existsSync(e) || t.printErrorExit("SECTION_ID_ERROR", ["Material not found!"]);
    const r = fs_1.readdirSync(e).filter(e => e !== SessionIdHelper.macDSStore);
    3 !== r.length && t.printErrorExit("CACHE_MATERIAL_ERROR", [(0, hvigor_user_home_js_1.getHvigorUserHomeCacheDir)()]);
    const n = [];
    return r.forEach(r => n.push(SessionIdHelper.readDirBytes(path_1.resolve(e, r), t))), n;
  }
  static readSalt(e, t) {
    return SessionIdHelper.readDirBytes(e, t);
  }
  static readWorkMaterial(e, t) {
    return SessionIdHelper.readDirBytes(e, t);
  }
  static readDirBytes(e, t) {
    fs_1.existsSync(e) && fs_1.statSync(e).isDirectory() || t.printErrorExit("SECTION_ID_ERROR", [`Can not find the signing material: ${e}`]);
    const r = fs_1.readdirSync(e).filter(e => e !== SessionIdHelper.macDSStore);
    1 !== r.length && t.printErrorExit("CACHE_MATERIAL_ERROR", [(0, hvigor_user_home_js_1.getHvigorUserHomeCacheDir)()]);
    const n = fs_1.readFileSync(path_1.resolve(e, r[0]));
    return new Int8Array(n);
  }
  static createMaterial(e) {
    e.debug("begin to create new key files");
    const t = path_1.join(daemon_const_js_1.DEFAULT_DAEMON_META_PATH);
    fs_extra_1.existsSync(t) || fs_extra_1.mkdirsSync(t), fs_1.chmodSync(path_1.join(t), daemon_const_js_1.KEY_FILE_DIRECTORY_PERMISSIONS);
    const r = path_1.join(t, this.dirs[0]),
      n = SessionIdHelper.createAndStoreKey(path_1.join(r, "0"), daemon_const_js_1.DAEMON_ROOT_KEY_COMPONENT_LENGTH),
      o = SessionIdHelper.createAndStoreKey(path_1.join(r, "1"), daemon_const_js_1.DAEMON_ROOT_KEY_COMPONENT_LENGTH),
      s = SessionIdHelper.createAndStoreKey(path_1.join(r, "2"), daemon_const_js_1.DAEMON_ROOT_KEY_COMPONENT_LENGTH),
      a = SessionIdHelper.createAndStoreKey(path_1.join(t, this.dirs[1]), daemon_const_js_1.DAEMON_SALT_KEY_LENGTH),
      _ = SessionIdHelper.getRootKey([n, o, s], a, e);
    SessionIdHelper.createAndStoreEnKey(_, path_1.join(t, this.dirs[2]));
  }
  static createAndStoreKey(e, t) {
    const r = new Int8Array((0, crypto_1.randomBytes)(16));
    fs_extra_1.existsSync(e) || fs_extra_1.mkdirpSync(e);
    const n = path_1.join(e, (0, hvigor_common_1.hash)(r));
    return fs_1.writeFileSync(n, r), fs_extra_1.chmodSync(n, daemon_const_js_1.KEY_FILE_PERMISSIONS), r;
  }
  static createAndStoreEnKey(e, t) {
    const r = new Int8Array((0, crypto_1.randomBytes)(daemon_const_js_1.DAEMON_WORK_KEY_LENGTH)),
      n = SessionIdHelper.encrypt(e, r);
    fs_extra_1.existsSync(t) || fs_extra_1.mkdirpSync(t);
    const o = path_1.join(t, (0, hvigor_common_1.hash)(n));
    fs_1.writeFileSync(o, n), fs_1.chmodSync(o, daemon_const_js_1.KEY_FILE_PERMISSIONS);
  }
}
SessionIdHelper.dirs = ["fd", "ac", "ce"], SessionIdHelper.macDSStore = ".DS_Store", SessionIdHelper.component = new Int8Array([49, 243, 9, 115, 214, 175, 91, 184, 211, 190, 177, 88, 101, 131, 192, 119]), SessionIdHelper.key = SessionIdHelper.getKey(daemon_const_js_1.DEFAULT_DAEMON_META_PATH, log);
