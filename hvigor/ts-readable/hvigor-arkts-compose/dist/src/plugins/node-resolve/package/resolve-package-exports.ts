/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/package/resolve-package-exports.js
 */
import * as resolve_package_imports_exports_js_1 from "./resolve-package-imports-exports";
import * as resolve_package_target_js_1 from "./resolve-package-target";
import * as utils_js_1 from "./utils";
export async function resolvePackageExports(e, s, r) {
  if ((0, utils_js_1.isMixedExports)(r)) throw new utils_js_1.InvalidConfigurationError(e, "All keys must either start with './', or without one.");
  if ("." === s) {
    let s;
    if ("string" == typeof r || Array.isArray(r) || (0, utils_js_1.isConditions)(r) ? s = r : (0, utils_js_1.isMappings)(r) && (s = r["."]), s) {
      const r = await (0, resolve_package_target_js_1.resolvePackageTarget)(e, {
        target: s,
        subpath: ""
      });
      if (r) return r;
    }
  } else if ((0, utils_js_1.isMappings)(r)) {
    const t = await (0, resolve_package_imports_exports_js_1.resolvePackageImportsExports)(e, {
      matchKey: s,
      matchObj: r
    });
    if (t) return t;
  }
  throw new utils_js_1.InvalidModuleSpecifierError(e);
}
