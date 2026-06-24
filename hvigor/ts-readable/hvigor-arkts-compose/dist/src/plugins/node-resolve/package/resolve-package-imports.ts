/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/package/resolve-package-imports.js
 */
import * as url_1 from "url";
import * as resolve_package_imports_exports_js_1 from "./resolve-package-imports-exports";
import * as utils_js_1 from "./utils";
export async function resolvePackageImports({
  importSpecifier: e,
  importer: r,
  moduleDirs: o,
  conditions: s,
  resolveId: t
}) {
  const i = await (0, utils_js_1.findPackageJson)(r, o);
  if (!i) throw new Error((0, utils_js_1.createBaseErrorMsg)(". Could not find a parent oh-package.json5."));
  const {
      pkgPath: a,
      pkgJsonPath: p,
      pkgJson: l
    } = i,
    n = {
      importer: r,
      importSpecifier: e,
      moduleDirs: o,
      pkgURL: (0, url_1.pathToFileURL)(`${a}/`),
      pkgJsonPath: p,
      conditions: s,
      resolveId: t
    },
    {
      imports: c
    } = l;
  if (!c) throw new utils_js_1.InvalidModuleSpecifierError(n, true);
  if ("#" === e || e.startsWith("#/")) throw new utils_js_1.InvalidModuleSpecifierError(n, true, "Invalid import specifier.");
  return (0, resolve_package_imports_exports_js_1.resolvePackageImportsExports)(n, {
    matchKey: e,
    matchObj: c,
    internal: true
  });
}
