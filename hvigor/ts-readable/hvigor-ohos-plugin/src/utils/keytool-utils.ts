/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/keytool-utils.js
 */
import * as child_process_1 from "child_process";
import * as file_util_1 from "./file-util";
export class KeytoolUtils {
  constructor(t, e, s, i) {
    this.storetype = "PKCS12", this.keytool = "keytool", this.storeFile = t, this.storePassword = e, this.keyAlias = s, this.dn = i, this.baseArgs = ["-noprompt", "-keystore", this.storeFile, "-storepass", this.storePassword, "-storetype", this.storetype];
  }
  generateKeyPair(t, e, s, i, r) {
    const l = ["-genkeypair", "-alias", this.keyAlias, "-dname", this.dn];
    return this.addArg(l, "-validity", t), this.addArg(l, "-keysize", e), this.addArg(l, "-keyalg", s), this.addArg(l, "-sigalg", i), this.addArg(l, "-startdate", r), this.execute(l);
  }
  requestCert(t) {
    const e = ["-certreq", "-alias", this.keyAlias, "-dname", this.dn];
    return this.addArg(e, "-file", t), this.execute(e);
  }
  generateCert(t, e, s, i, r, l) {
    const a = ["-gencert", "-alias", this.keyAlias];
    this.addArg(a, "-dname", this.dn), this.addArg(a, "-validity", e), this.addArg(a, "-sigalg", s), this.addArg(a, "-startdate", i), this.addArg(a, "-infile", r), this.addArg(a, "-outfile", l), t && a.push("-rfc");
    const o = this.execute(a);
    return file_util_1.FileUtil.deleteFile(r), o;
  }
  importCert(t, e) {
    const s = ["-importcert", "-alias", this.keyAlias];
    t && s.push("-trustcacerts"), this.addArg(s, "-file", e);
    const i = this.execute(s);
    return file_util_1.FileUtil.deleteFile(e), i;
  }
  execute(t) {
    t = t.concat(this.baseArgs);
    return !(0, child_process_1.spawnSync)(this.keytool, t).error;
  }
  addArg(t, e, s) {
    if (undefined !== s) if (s instanceof Date) {
      const i = `${s.getFullYear()}/${`0${s.getMonth() + 1}`.slice(-2)}/${`0${s.getDate()}`.slice(-2)} ${s.toTimeString().substring(0, 8)}`;
      t.push(e), t.push(i);
    } else t.push(e), t.push(`${s}`);
  }
}
