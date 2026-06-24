/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/package/utils.js
 */
import fs_1 from "fs";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import path_1 from "path";
import * as fs_js_1 from "../../../util/fs";
function isModuleDir(r, e) {
  return e.some(e => r.endsWith(e));
}
export async function findPackageJson(r, e) {
  const {
    root: o
  } = path_1.parse(r);
  let s = r;
  for (; s !== o && !isModuleDir(s, e);) {
    const r = path_1.join(s, "oh-package.json5");
    if (await (0, fs_js_1.fileExists)(r)) {
      const e = fs_1.readFileSync(r, "utf-8");
      return {
        pkgJson: (0, hvigor_common_1.parseJsonText)(e),
        pkgPath: s,
        pkgJsonPath: r
      };
    }
    s = path_1.resolve(s, "..");
  }
  return null;
}
export function isUrl(r) {
  try {
    return !!new URL(r);
  } catch (r) {
    return false;
  }
}
export function isConditions(r) {
  return "object" == typeof r && Object.keys(r).every(r => !r.startsWith("."));
}
export function isMappings(r) {
  return "object" == typeof r && !isConditions(r);
}
export function isMixedExports(r) {
  const e = Object.keys(r);
  return e.some(r => r.startsWith(".")) && e.some(r => !r.startsWith("."));
}
export function createBaseErrorMsg(r, e) {
  return `Could not resolve import "${r}" in ${e}`;
}
export function createErrorMsg(r, e, o) {
  const {
    importSpecifier: s,
    importer: t,
    pkgJsonPath: i
  } = r;
  return `${createBaseErrorMsg(s, t)} using ${o ? "imports" : "exports"} defined in ${i}.${e ? ` ${e}` : ""}`;
}
export class ResolveError extends Error {}
export class InvalidConfigurationError extends ResolveError {
  constructor(r, e) {
    super(createErrorMsg(r, `Invalid "exports" field. ${e}`));
  }
}
export class InvalidModuleSpecifierError extends ResolveError {
  constructor(r, e, o) {
    super(createErrorMsg(r, o, e));
  }
}
export class InvalidPackageTargetError extends ResolveError {
  constructor(r, e) {
    super(createErrorMsg(r, e));
  }
}
