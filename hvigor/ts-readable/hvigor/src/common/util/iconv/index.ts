/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/iconv/index.js
 */
import * as index_js_1 from "./encodings/index";
import * as bom_handling_js_1 from "./bom-handling";
class Iconv {
  constructor() {
    this.codecDataCache = {};
  }
  decode(e, n, o) {
    var c;
    const r = this.getDecoder(n, o),
      t = Buffer.isBuffer(e) ? e : Buffer.from(e);
    return r.write(t) + (null !== (c = r.end()) && undefined !== c ? c : "");
  }
  encode(e, n, o) {
    const c = this.getEncoder(n, o),
      r = c.write(e),
      t = c.end();
    return t && t.length > 0 ? Buffer.concat([r, t]) : r;
  }
  encodingExists(e) {
    return Object.prototype.hasOwnProperty.call(index_js_1.encodings, this.canonicalizeEncoding(e));
  }
  getDecoder(e, n) {
    const o = this.getCodec(e);
    let c = new o.decoder(n, o);
    return !o.bomAware || n && false === n.stripBOM || (c = new bom_handling_js_1.StripBOMDecoderWrapper(c, n)), c;
  }
  getEncoder(e, n) {
    const o = this.getCodec(e);
    let c = new o.encoder(n, o);
    return o.bomAware && n && n.addBOM && (c = new bom_handling_js_1.PrependBOMEncoderWrapper(c, n)), c;
  }
  getCodec(e) {
    let n = this.canonicalizeEncoding(e),
      o = {},
      c = this.codecDataCache[n];
    for (; undefined === c;) {
      const r = index_js_1.encodings[n];
      switch (typeof r) {
        case "string":
          n = r;
          break;
        case "object":
          o = {
            ...o,
            ...r
          }, o.encodingName || (o.encodingName = n), n = r.type;
          break;
        case "function":
          o.encodingName || (o.encodingName = n), c = new r(o), this.codecDataCache[o.encodingName] = c;
          break;
        default:
          throw new Error(`Encoding not recognized: '${e}' (searched as: '${n}')`);
      }
    }
    return c;
  }
  canonicalizeEncoding(e) {
    return e.toLowerCase().replace(/:\d{4}$|[^0-9a-z]/g, "");
  }
}
export const iconv = new Iconv();
