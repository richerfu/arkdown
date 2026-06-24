/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/wrapper-chunk-plugin.js
 */
import * as index_js_1 from "../util/magic-string-replacement/index";
const VARIABLE_DECLARATION = "VariableDeclaration",
  CALL_EXPRESSION = "CallExpression",
  EXPRESSION_STATEMENT = "ExpressionStatement",
  REQUIRE_NAME = "require",
  VAR_NAME = "var";
export function wrapperChunkPlugin() {
  return {
    name: "wrapperChunkPlugin",
    renderChunk: renderChunk
  };
}
function renderChunk(e, n, r, {
  chunks: t
}) {
  const o = new index_js_1.MagicStringReplacement(e);
  return (this.share.projectConfig.supportChunks ?? false) && analyzeImportAndExport(this.parse, n, e, o, t), o.appendLeft(0, "(function () {\n"), o.append("\n})()"), {
    code: o.toString(),
    map: r.sourcemap ? o.generateMap({
      hires: true
    }) : null
  };
}
function analyzeImportAndExport(e, n, r, t, o) {
  0 !== n.imports.length && analyzeImport(e(r), n, t, o);
  0 !== n.exports.length && analyzeExport(n, t);
}
function analyzeImport(e, n, r, t) {
  const o = new Set();
  for (const e of n.imports) 0 === n.importedBindings[e].length && o.add(e);
  for (const a of e.body) a.type === VARIABLE_DECLARATION && "var" === a.kind && replaceRequireVariable(a, n, r, t), a.type === EXPRESSION_STATEMENT && 0 !== o.size && a.expression.type === CALL_EXPRESSION && "require" === a.expression.callee.name && removeEmptyRequire(a, n, o, r);
}
function replaceRequireVariable(e, n, r, t) {
  for (const o of e.declarations) {
    const e = o.init;
    if (!e || e.type !== CALL_EXPRESSION || "require" !== e.callee?.name) continue;
    let a;
    for (const e of o.init.arguments) {
      const r = e.value;
      if (n.importsPathMap.get(r)) {
        a = n.importsPathMap.get(r);
        break;
      }
    }
    a && r.overwrite(e.start, e.end, `globalThis['${a}_${t[a]?.chunkHash}']`);
  }
}
function removeEmptyRequire(e, n, r, t) {
  for (const o of e.expression.arguments) {
    const a = o.value,
      i = n.importsPathMap.get(a);
    if (r.has(i)) {
      t.remove(e.start, e.end);
      break;
    }
  }
}
function analyzeExport(e, n) {
  const r = [];
  for (const [n] of e.exportsByName) r.push(`${n}: exports.${n}`);
  n.append(`\nglobalThis['${e.fileName}_${e?.chunkHash}'] \n        = globalThis['${e.fileName}_${e?.chunkHash}'] || {\n  ${r.join(",\n  ")}\n}`);
}
