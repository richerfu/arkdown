/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/version/sdk-version.js
 */
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
export class SdkVersion {
  constructor(i) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(SdkVersion.name), this.version = i;
    const r = i.split(".");
    4 !== r.length && this._log.printErrorExit("INVALID_SDK_VERSION"), this._major = parseInt(r[0]), this._minor = parseInt(r[1]), this._revision = parseInt(r[2]), this._build = parseInt(r[3]);
  }
  getVersion() {
    return this.version;
  }
  getMajor() {
    return this._major;
  }
  getMinor() {
    return this._minor;
  }
  getRevision() {
    return this._revision;
  }
  getBuild() {
    return this._build;
  }
  isHigherThan(i) {
    return this._major !== i.getMajor() ? this._major > i._major : this._minor !== i.getMinor() ? this._minor > i.getMinor() : this._revision !== i.getRevision() ? this._revision > i.getRevision() : this._build !== i.getBuild() && this._build > i.getBuild();
  }
  equals(i) {
    return this._build === i.getBuild() && this._revision === i.getRevision() && this._minor === i.getMinor() && this._major === i.getMajor();
  }
}
