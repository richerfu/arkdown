/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/package/resolve-package-target.js
 */
import * as url_1 from "url";
import * as utils_js_1 from "./utils";
function includesInvalidSegments(e, r) {
  return e.split("/").slice(1).some(e => [".", "..", ...r].includes(e));
}
export async function resolvePackageTarget(e, {
  target: r,
  subpath: t,
  pattern: a,
  internal: n
}) {
  if ("string" == typeof r) return stringHandler(a, t, r, e, n);
  if (Array.isArray(r)) return arrayHandler(a, t, r, e, n);
  if (r && "object" == typeof r) return objectHandler(a, t, r, e, n);
  if (null === r) return null;
  throw new utils_js_1.InvalidPackageTargetError(e, "Invalid exports field.");
}
async function stringHandler(e, r, t, a, n) {
  if (!e && r.length > 0 && !t.endsWith("/")) throw new utils_js_1.InvalidModuleSpecifierError(a);
  if (!t.startsWith("./")) {
    if (n && !["/", "../"].some(e => t.startsWith(e)) && !(0, utils_js_1.isUrl)(t)) {
      if (e) {
        const e = await a.resolveId(t.replace(/\*/g, r), a.pkgURL.href);
        return e ? (0, url_1.pathToFileURL)(e.location).href : null;
      }
      const n = await a.resolveId(`${t}${r}`, a.pkgURL.href);
      return n ? (0, url_1.pathToFileURL)(n.location).href : null;
    }
    throw new utils_js_1.InvalidPackageTargetError(a, `Invalid mapping: '${t}'.`);
  }
  if (includesInvalidSegments(t, a.moduleDirs)) throw new utils_js_1.InvalidPackageTargetError(a, `Invalid mapping: '${t}'.`);
  const i = new URL(t, a.pkgURL);
  if (!i.href.startsWith(a.pkgURL.href)) throw new utils_js_1.InvalidPackageTargetError(a, `Resolved to ${i.href} which is outside package: ${a.pkgURL.href}`);
  if (includesInvalidSegments(r, a.moduleDirs)) throw new utils_js_1.InvalidModuleSpecifierError(a);
  return e ? i.href.replace(/\*/g, r) : new URL(r, i).href;
}
async function arrayHandler(e, r, t, a, n) {
  let i;
  for (const s of t) try {
    const t = await resolvePackageTarget(a, {
      target: s,
      subpath: r,
      pattern: e,
      internal: n
    });
    if (undefined !== t) return t;
  } catch (e) {
    if (!(e instanceof utils_js_1.InvalidPackageTargetError)) throw e;
    i = e;
  }
  if (i) throw i;
  return null;
}
async function objectHandler(e, r, t, a, n) {
  for (const [i, s] of Object.entries(t)) if ("default" === i || a.conditions.includes(i)) {
    const t = await resolvePackageTarget(a, {
      target: s,
      subpath: r,
      pattern: e,
      internal: n
    });
    if (undefined !== t) return t;
  }
}
