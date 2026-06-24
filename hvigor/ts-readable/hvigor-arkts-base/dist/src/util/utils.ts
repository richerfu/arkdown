/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/util/utils.js
 */
import fs_1 from "fs";
import * as path from "path";
export const findSuitableFilePath = (e, t) => {
  if (!!path.extname(e) && fs_1.existsSync(e)) return e;
  const i = [];
  fs_1.existsSync(e) && fs_1.statSync(e).isDirectory() ? i.push(path.join(e, "Index"), path.join(e, "index")) : i.push(e);
  for (const e of t) for (const t of i) if (fs_1.existsSync(`${t}${e}`)) return `${t}${e}`;
  return "";
};
