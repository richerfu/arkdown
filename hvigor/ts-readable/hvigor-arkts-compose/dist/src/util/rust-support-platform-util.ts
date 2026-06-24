/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/rust-support-platform-util.js
 */
const supportedPlatforms = {
  linux: ["x64", "arm64"],
  darwin: ["x64", "arm64"],
  win32: ["x64"]
};
export function isSupportedPlatform() {
  const r = process.platform;
  if (!Object.prototype.hasOwnProperty.call(supportedPlatforms, r)) return false;
  const t = supportedPlatforms[r],
    o = process.arch;
  return -1 !== t.indexOf(o);
}
