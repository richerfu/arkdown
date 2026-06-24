/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/keystore-utils.js
 */
import * as fse from "fs-extra";
import path_1 from "path";
import * as decipher_util_1 from "./decipher-util";
import * as file_util_1 from "./file-util";
import * as keytool_utils_1 from "./keytool-utils";
export class KeyStoreHelper {
  static getDefaultDebugKeyStoreLocation() {
    const e = process.env.HOME || process.env.USERPROFILE;
    if (undefined === e) return;
    const t = path_1.resolve(e, KeyStoreHelper.DEFAULT_IDE_CONFIG_PATH, KeyStoreHelper.CONFIG);
    return fse.existsSync(t) || fse.mkdirsSync(t), path_1.resolve(t, KeyStoreHelper.DEFAULT_DEBUG_SIGN_FILE);
  }
  static createDefaultDebugStore() {
    const e = KeyStoreHelper.getDefaultDebugKeyStoreLocation();
    undefined === e || fse.existsSync(e) || KeyStoreHelper.createDebugStore(e, KeyStoreHelper.DEFAULT_STORE_PASS, KeyStoreHelper.DEFAULT_ALIAS) || (file_util_1.FileUtil.deleteFile(KeyStoreHelper.requestCertFile), file_util_1.FileUtil.deleteFile(KeyStoreHelper.generateCertFile));
  }
  static createDebugStore(e, t, r) {
    return KeyStoreHelper.createNewStore(e, t, r, KeyStoreHelper.CERTIFICATE_DESC);
  }
  static createNewStore(e, t, r, l) {
    let o;
    const i = new keytool_utils_1.KeytoolUtils(e, t, r, l);
    return o = i.generateKeyPair(KeyStoreHelper.validity, KeyStoreHelper.keyBitsLength, KeyStoreHelper.keyAlgo, KeyStoreHelper.signatureAlgo, KeyStoreHelper.validFrom), o ? (o = i.requestCert(KeyStoreHelper.requestCertFile), o ? (o = i.generateCert(true, KeyStoreHelper.validity, KeyStoreHelper.signatureAlgo, KeyStoreHelper.validFrom, KeyStoreHelper.requestCertFile, KeyStoreHelper.generateCertFile), o ? (o = i.importCert(true, KeyStoreHelper.generateCertFile), o) : o) : o) : o;
  }
  static loadDefaultStorePass() {
    return decipher_util_1.DecipherUtil.decryptPluginDataFromResource(KeyStoreHelper.DEFAULT_STORE_PASS_FILE);
  }
}
KeyStoreHelper.CERTIFICATE_DESC = "CN=debugkey", KeyStoreHelper.DEFAULT_IDE_CONFIG_PATH = ".ohos", KeyStoreHelper.CONFIG = "config", KeyStoreHelper.DEFAULT_DEBUG_SIGN_FILE = "debug.p12", KeyStoreHelper.keyBitsLength = 3072, KeyStoreHelper.DEFAULT_STORE_PASS_FILE = "../../res/material/zb/de", KeyStoreHelper.DEFAULT_STORE_PASS = KeyStoreHelper.loadDefaultStorePass(), KeyStoreHelper.DEFAULT_ALIAS = "debugKey", KeyStoreHelper.validity = 365, KeyStoreHelper.validFrom = new Date(), KeyStoreHelper.keyAlgo = "RSA", KeyStoreHelper.signatureAlgo = "SHA256withRSA", KeyStoreHelper.requestCertFile = "requestCert.req", KeyStoreHelper.generateCertFile = "generateCert.gen";
