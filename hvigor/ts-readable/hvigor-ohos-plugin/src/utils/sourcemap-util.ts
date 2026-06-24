/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/sourcemap-util.js
 */
import * as fse from "fs-extra";
import * as ohos_logger_js_1 from "./log/ohos-logger";
const logger = ohos_logger_js_1.OhosLogger.getLogger("SourceMap");
export async function overWriteSourceMap(e, r) {
  if (fse.existsSync(e) && fse.existsSync(r)) try {
    let t = await fse.readFile(r, "utf8");
    t.endsWith(",") && (t = t.substring(0, t.length - 1)), t = `{${t}}`;
    const o = JSON.parse(t),
      s = await fse.readJSON(e);
    Object.keys(o).forEach(e => {
      const r = o[e].mappings;
      if (r) for (let t of Object.values(s)) {
        const o = t;
        if (o.sources && 1 === o.sources.length && o.sources[0] === e) {
          o.mappings = r;
          break;
        }
      }
    }), await fse.writeJSON(e, s, {
      spaces: 2,
      encoding: "utf8"
    });
  } catch (e) {
    logger.debug(`Error occurred: ${e instanceof Error ? e.message : "Write sourceMap with unknown error"}`);
  }
}
