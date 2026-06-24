/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/local-file-writer.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as replacer_js_1 from "./replacer";
export class LocalFileWriter {
  constructor() {
    this._replacer = replacer_js_1.replacer, this._space = 2;
  }
  withSpace(e) {
    this._space = e;
  }
  withReplacer(e) {
    this._replacer = e;
  }
  write(e, t) {
    try {
      const r = JSON.stringify(t, this._replacer, this._space);
      this.writeStr(e, r);
    } catch (r) {
      const i = this.chunkStringify(t);
      this.writeStrArr(e, i);
    }
  }
  chunkStringify(e) {
    const t = Object.keys(e),
      r = ["{\n"],
      i = new Array(this._space).fill(" ").join("");
    return t.forEach(t => {
      if (Array.isArray(e[t]) && e[t].length) {
        r.push(`${i}${JSON.stringify(t)}: [\n`);
        let s = "";
        for (let a = 0; a < e[t].length; a++) {
          const c = e[t][a],
            n = `${JSON.stringify(c, this._replacer, this._space).split("\n").map(e => `${i}${i}${e}`).join("\n")},\n`;
          s.length >= 1e8 ? (r.push(s), s = n) : s += n;
        }
        r.push(`${s.replace(/,\n$/, "\n")}${i}],\n`);
      } else r.push(`${i}${JSON.stringify(t)}: ${JSON.stringify(e[t], this._replacer, this._space)},\n`);
    }), r[r.length - 1] = r[r.length - 1].replace(/,\n$/, "\n"), r.push("}"), r;
  }
  writeStr(e, t) {
    const r = path_1.dirname(e);
    fs_extra_1.existsSync(r) || fs_extra_1.mkdirSync(r, {
      recursive: true
    }), fs_extra_1.writeFileSync(e, t);
  }
  writeStrArr(e, t) {
    const r = path_1.dirname(e);
    fs_extra_1.existsSync(r) || fs_extra_1.mkdirSync(r, {
      recursive: true
    }), fs_extra_1.writeFileSync(e, ""), t.forEach(t => {
      fs_extra_1.appendFileSync(e, t);
    });
  }
  static getInstance() {
    return LocalFileWriter.instance || (LocalFileWriter.instance = new LocalFileWriter()), LocalFileWriter.instance;
  }
}
