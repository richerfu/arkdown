/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/json-schema-util.js
 */
import os_1 from "os";
import util_1 from "util";
import ajv_1 from "ajv";
const ajv = new ajv_1({
  strict: false,
  allErrors: true
});
export function validateDataBySchema(a, e, t) {
  let r = ajv.getSchema(e);
  r || (ajv.addSchema(t, e), r = ajv.getSchema(e));
  const s = r(a);
  if (s || !r.errors) return {
    isValid: s,
    errMsg: ""
  };
  const i = [];
  r.errors.forEach(a => {
    const e = {
      instancePath: a.instancePath,
      keyword: a.keyword,
      params: a.params,
      message: a.message
    };
    i.push(e);
  });
  return {
    isValid: s,
    errMsg: i.map(a => util_1.format(a, os_1.EOL)).join("")
  };
}
