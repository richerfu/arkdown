/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/preview-resource-watcher-plugin.js
 */
export function previewResourceWatcherPlugin(e) {
  return {
    name: "previewResourceWatcherPlugin",
    buildStart() {
      e.forEach(e => this.addWatchFile(e));
    }
  };
}
