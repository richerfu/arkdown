/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/magic-string-replacement/sourcemap-builder.js
 */
import * as source_map_1 from "source-map";
export class SourceMapBuilder {
  constructor(e, t, i) {
    this.curGeneratedLine = 0, this.curGeneratedColumn = 0, this.decodedMappings = [], this.sourceCode = e, this.firstCodeFragment = t, this.splitSourceCode = e.split("\n"), this.sourceCodePrefix = i, this.moveLineColumnPointer(this.sourceCodePrefix);
    let n = this.firstCodeFragment;
    for (; n;) this.moveLineColumnPointer(n.prefix), this.resolveCodeFragment(n), this.moveLineColumnPointer(n.suffix), n = n.next;
  }
  generateMap() {
    const e = new source_map_1.SourceMapGenerator({
      skipValidation: true
    });
    return this.decodedMappings.forEach((t, i) => {
      t.forEach(t => {
        const [n,, r, s] = t;
        e.addMapping({
          original: {
            line: r + 1,
            column: s
          },
          generated: {
            line: i + 1,
            column: n
          },
          source: ""
        });
      });
    }), {
      mappings: e.toJSON().mappings,
      version: 3,
      names: [],
      file: null,
      sourcesContent: [null],
      sources: [null],
      toString() {
        return JSON.stringify(this);
      },
      toUrl() {
        return `data:application/json;charset=utf-8;base64,${Buffer.from(this.toString(), "utf-8").toString("base64")}`;
      }
    };
  }
  resolveCodeFragment(e) {
    let {
      sourceLine: t,
      sourceColumn: i
    } = this.offsetToLineColumn(e.start);
    if (e.isOverwritten && e.content.length > 0) this.decodedMappings[this.curGeneratedLine] || (this.decodedMappings[this.curGeneratedLine] = []), this.decodedMappings[this.curGeneratedLine].push([this.curGeneratedColumn, 0, t, i]), this.moveLineColumnPointer(e.content);else if (!e.isOverwritten) for (let n = e.start; n < e.end; n++) this.decodedMappings[this.curGeneratedLine] || (this.decodedMappings[this.curGeneratedLine] = []), this.decodedMappings[this.curGeneratedLine].push([this.curGeneratedColumn, 0, t, i]), "\n" === this.sourceCode.at(n) ? (t++, i = 0, this.curGeneratedLine++, this.curGeneratedColumn = 0, this.decodedMappings[this.curGeneratedLine] = []) : (i++, this.curGeneratedColumn++);
  }
  offsetToLineColumn(e) {
    let t,
      i = e;
    for (t = 0; t < this.splitSourceCode.length; t++) {
      const e = this.splitSourceCode[t].length;
      if (i <= e) break;
      i -= e + 1;
    }
    return {
      sourceLine: t,
      sourceColumn: i
    };
  }
  moveLineColumnPointer(e) {
    for (let t = 0; t < e.length; t++) {
      "\n" === e.charAt(t) ? (this.curGeneratedLine++, this.decodedMappings[this.curGeneratedLine] = [], this.curGeneratedColumn = 0) : this.curGeneratedColumn++;
    }
  }
}
