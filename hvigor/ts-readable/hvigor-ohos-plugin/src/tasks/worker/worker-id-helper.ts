/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/worker-id-helper.js
 */
let moduleIndex = 0;
const moduleIndices = new Map();
export function getModuleIndex(e, t = false) {
  const o = generateUniqueKey(e, t);
  return moduleIndices.has(o) || moduleIndices.set(o, moduleIndex++), moduleIndices.get(o);
}
export function getWorkerIdWithModule(e, t = false) {
  const o = generateUniqueKey(e, t);
  return moduleIndices.get(o);
}
export function setWorkerIdWithModule(e, t, o = false) {
  const d = generateUniqueKey(e, o);
  moduleIndices.set(d, t);
}
function generateUniqueKey(e, t = false) {
  return `${e}:${t}`;
}
