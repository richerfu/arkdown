/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/wait-async-plugin.js
 */
export function waitAsyncPlugin() {
  return {
    name: "waitAsyncPlugin",
    async bundleEnd() {
      await this.block(), this.releaseDonePromise();
    }
  };
}
