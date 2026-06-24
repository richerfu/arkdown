/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/decipher-util.js
 */
import buffer_1 from "buffer";
import crypto_1 from "crypto";
import fs_1 from "fs";
import path_1 from "path";
import * as ohos_logger_js_1 from "./log/ohos-logger";
var _a;
export class DecipherUtil {
  static decryptPwd(t, r, e) {
    this.materialDir = t, r.length < 32 && this._logger.printErrorExit("INVALID_DATA", [e], [[e]]), r.length % 2 != 0 && this._logger.printErrorExit("INVALID_PASSWORD_LENGTH", [e], [[e]]);
    let i = buffer_1.Buffer.from("");
    const s = DecipherUtil.getKey(t, e),
      _ = new Int8Array(buffer_1.Buffer.from(r, "hex"));
    return i = DecipherUtil.decrypt(s, _, e), i.toString("utf-8");
  }
  static getKey(t, r) {
    const e = path_1.resolve(t, "material");
    fs_1.statSync(e).isDirectory() || this._logger.printErrorExit("SIGNING_FAILED_NO_SIGNING_NATERIALS_IS_NOT_A_DIRECTORY", [e, r], [[e], [r], [t]]);
    const i = fs_1.readdirSync(e).filter(t => t !== this.macDSStore);
    0 === i.length && this._logger.printErrorExit("SIGNING_FAILED_NO_SIGNING_NATERIALS_IS_AN_EMPTY_DIRECTORY", [e, r], [[e], [r], [t]]), this.dirs.forEach(s => {
      i.includes(s) || this._logger.printErrorExit("SIGNING_FAILED_CAN_NOT_FIND_SIGNING_MATERIAL", [s, r], [[this.dirs.join("、"), e], [r], [t]]);
    });
    const s = this.readFd(e, this.dirs[0], r),
      _ = this.readSalt(path_1.resolve(e, this.dirs[1]), r),
      a = this.getRootKey(s, _, r),
      o = this.readWorkMaterial(path_1.resolve(e, this.dirs[2]), r);
    return new Int8Array(DecipherUtil.decrypt(a, o, r));
  }
  static getRootKey(t, r, e) {
    const i = t.concat(this.component),
      s = this.xorComponents(i, e),
      _ = crypto_1.pbkdf2Sync(s.toString(), r, 1e4, 16, "sha256");
    return new Int8Array(_);
  }
  static decrypt(t, r, e) {
    try {
      const e = (255 & r[0]) << 24 | (255 & r[1]) << 16 | (255 & r[2]) << 8 | 255 & r[3],
        i = r.length - 4 - e,
        s = r.slice(4, 4 + i),
        _ = crypto_1.createDecipheriv("aes-128-gcm", t, s),
        a = r.slice(r.length - 16);
      _.setAuthTag(a);
      const o = _.update(r.subarray(4 + i, r.length - 16)),
        l = _.final();
      return buffer_1.Buffer.concat([o, l]);
    } catch (t) {
      this._logger.printErrorExit("KEY_PASSWORD_VALID_FAILED", [t.message, e], [[]]);
    }
    return buffer_1.Buffer.alloc(0);
  }
  static xorComponents(t, r) {
    t.forEach(t => {
      16 !== t.length && this._logger.printErrorExit("SIGNING_FAILED_SIGZNING_MATERIAL_DATA_ERROR", [r], [[r], [this.materialDir]]);
    });
    let e = this.xor(t[0], t[1], r);
    for (let i = 2; i < t.length; i++) e = this.xor(e, t[i], r);
    return buffer_1.Buffer.from(e);
  }
  static xor(t, r, e) {
    t.byteLength !== r.byteLength && this._logger.printErrorExit("SIGNING_FAILED_SIGZNING_MATERIAL_DATA_ERROR", [e], [[e], [this.materialDir]]);
    const i = new Int8Array(t.byteLength);
    for (let e = 0; e < t.byteLength; e++) i[e] = t[e] ^ r[e];
    return i;
  }
  static readFd(t, r, e) {
    const i = path_1.resolve(t, r);
    fs_1.existsSync(i) || this._logger.printErrorExit("SIGNING_FAILED_CAN_NOT_FIND_SIGNING_MATERIAL", [r, e], [[this.dirs.join("、"), t], [e], [this.materialDir]]);
    const s = fs_1.readdirSync(i).filter(t => t !== this.macDSStore);
    3 !== s.length && this._logger.printErrorExit("SIGNING_FAILED_SIGNING_MATERIAL_IS_ILLEGAL", [i, e], [[i], [e], [this.materialDir]]);
    const _ = [];
    return s.forEach(t => _.push(this.readDirBytes(path_1.resolve(i, t), e))), _;
  }
  static readSalt(t, r) {
    return this.readDirBytes(t, r);
  }
  static readWorkMaterial(t, r) {
    return this.readDirBytes(t, r);
  }
  static decryptPluginDataFromResource(t) {
    const r = path_1.resolve(__dirname, t),
      e = fs_1.readFileSync(r).toString();
    return DecipherUtil.decryptPwd(path_1.resolve(__dirname, "../../res"), e, r);
  }
  static getProjectBuildProfilePath() {
    return path_1.resolve(process.cwd(), "build-profile.json5");
  }
}
_a = DecipherUtil, DecipherUtil.component = new Int8Array([49, 243, 9, 115, 214, 175, 91, 184, 211, 190, 177, 88, 101, 131, 192, 119]), DecipherUtil.dirs = ["fd", "ac", "ce"], DecipherUtil._logger = ohos_logger_js_1.OhosLogger.getLogger("Decipher"), DecipherUtil.macDSStore = ".DS_Store", DecipherUtil.readDirBytes = (t, r) => {
  fs_1.existsSync(t) && fs_1.statSync(t).isDirectory() || DecipherUtil._logger.printErrorExit("SIGNING_FAILED_CAN_NOT_FIND_THE_SIGZNING_MATERIAL", [r], [[t], [r], [_a.materialDir]]);
  const e = fs_1.readdirSync(t);
  1 !== e.length && DecipherUtil._logger.printErrorExit("SIGNING_FAILED_SIGZNING_MATERIAL_ERROR", [r], [[t], [r], [_a.materialDir]]);
  const i = fs_1.readFileSync(path_1.resolve(t, e[0]));
  return new Int8Array(i);
};
