/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/work-scripts/compile-code-snippet.js
 */
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import fs_1 from "fs";
import * as path from "path";
const logger = hvigor_arkts_base_1.ArktsBaseLogger.getLogger("compile-code-snippet");
let server;
export function compileCodeSnippet(e) {
  return !!server && (server.compileComponent(e), true);
}
export function initServer(e, t, r, i) {
  if (server) return;
  const o = path.resolve(r, "server", "build_pipe_server.js");
  fs_1.existsSync(o) ? (server = require(o).buildPipeServer(), server.init(e, t, e => {
    i.postMessage({
      type: hvigor_arkts_base_1.WatchEvent.WATCH_COMPILE_RESULT,
      content: e
    });
  })) : logger.error("Build pipe server not found.");
}
