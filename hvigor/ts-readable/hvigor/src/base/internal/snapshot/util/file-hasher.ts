/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/file-hasher.js
 */
import * as fs from "fs";
import * as path from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
export class FileContentHasher {
  hash(e) {
    const t = fs.readFileSync(e);
    return (0, hvigor_common_1.hash)(t);
  }
}
export class FileMetaHasher {
  hash(e, t) {
    return (0, hvigor_common_1.hash)(path.basename(e) + t.size + t.lastModifiedTime);
  }
}
