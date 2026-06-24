/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/ci-util.js
 */
export function isCI() {
  return !("false" === process.env.CI || !(process.env.BUILD_ID || process.env.BUILD_NUMBER || process.env.CI || process.env.CI_APP_ID || process.env.CI_BUILD_ID || process.env.CI_BUILD_NUMBER || process.env.CI_NAME || process.env.CONTINUOUS_INTEGRATION || process.env.RUN_ID || name));
}
