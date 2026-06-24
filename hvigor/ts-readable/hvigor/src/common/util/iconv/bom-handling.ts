/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/iconv/bom-handling.js
 */
const BOMChar = "\ufeff";
export class StripBOMDecoderWrapper {
  constructor(e, r) {
    this.decoder = e, this.pass = false, this.options = r || {};
  }
  write(e) {
    const r = Buffer.isBuffer(e) ? e : Buffer.from(e);
    let t = this.decoder.write(r);
    return this.pass || !t || ("\ufeff" === t[0] && (t = t.slice(1)), this.pass = true), t;
  }
  end() {
    return this.decoder.end();
  }
}
export class PrependBOMEncoderWrapper {
  constructor(e, r) {
    this.encoder = e, this.addBOM = true;
  }
  write(e) {
    return this.addBOM && (e = "\ufeff" + e, this.addBOM = false), this.encoder.write(e);
  }
  end() {
    return this.encoder.end();
  }
}
