/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/use-sensitive-check-plugin.js
 */
const CASE_SENSITIVE_CHECK = "caseSensitiveCheck";
export function sensitiveCheckPlugin() {
  return {
    name: "sensitiveCheckPlugin",
    shouldInvalidCache: shouldInvalidCache
  };
}
function shouldInvalidCache() {
  return !((this.cache?.get("caseSensitiveCheck") ?? false) || !this.share?.projectConfig?.caseSensitiveCheck);
}
