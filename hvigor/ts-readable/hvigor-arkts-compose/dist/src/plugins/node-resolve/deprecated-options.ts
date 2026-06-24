/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/deprecated-options.js
 */
function handleDeprecatedOptions(e) {
  const o = [];
  if (e.customResolveOptions) {
    const {
      customResolveOptions: r
    } = e;
    if (r.moduleDirectory && (e.moduleDirectories = Array.isArray(r.moduleDirectory) ? r.moduleDirectory : [r.moduleDirectory], o.push("node-resolve: The `customResolveOptions.moduleDirectory` option has been deprecated. Use `moduleDirectories`, which must be an array.")), r.preserveSymlinks) throw new Error("node-resolve: `customResolveOptions.preserveSymlinks` is no longer an option. We now always use the rollup `preserveSymlinks` option.");
    ["basedir", "package", "extensions", "includeCoreModules", "readFile", "isFile", "isDirectory", "realpath", "packageFilter", "pathFilter", "paths", "packageIterator"].forEach(e => {
      if (r[e]) throw new Error(`node-resolve: \`customResolveOptions.${e}\` is no longer an option. If you need this, please open an issue.`);
    });
  }
  return {
    warnings: o
  };
}
export default handleDeprecatedOptions;
