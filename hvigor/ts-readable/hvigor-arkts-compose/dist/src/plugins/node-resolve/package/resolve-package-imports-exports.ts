/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/package/resolve-package-imports-exports.js
 */
import * as resolve_package_target_js_1 from "./resolve-package-target";
import * as utils_js_1 from "./utils";
export async function resolvePackageImportsExports(e, {
  matchKey: t,
  matchObj: r,
  internal: s
}) {
  if (!t.endsWith("*") && t in r) {
    const a = r[t];
    return (0, resolve_package_target_js_1.resolvePackageTarget)(e, {
      target: a,
      subpath: "",
      internal: s
    });
  }
  const a = Object.keys(r).filter(e => e.endsWith("/") || e.endsWith("*")).sort((e, t) => t.length - e.length);
  for (const o of a) {
    const a = o.substring(0, o.length - 1);
    if (o.endsWith("*") && t.startsWith(a)) {
      const a = r[o],
        n = t.substring(o.length - 1);
      return (0, resolve_package_target_js_1.resolvePackageTarget)(e, {
        target: a,
        subpath: n,
        pattern: true,
        internal: s
      });
    }
    if (t.startsWith(o)) {
      const a = r[o],
        n = t.substring(o.length);
      return (0, resolve_package_target_js_1.resolvePackageTarget)(e, {
        target: a,
        subpath: n,
        internal: s
      });
    }
  }
  throw new utils_js_1.InvalidModuleSpecifierError(e, s);
}
