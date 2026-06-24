/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/boot/hooks/exit.js
 */
export function exit(e) {
  "win32" === process.platform && process.stdout.writableLength ? process.stdout.once("drain", function () {
    process.exit(e);
  }) : process.exit(e);
}
