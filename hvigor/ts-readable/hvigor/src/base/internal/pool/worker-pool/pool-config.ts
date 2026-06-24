/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/pool-config.js
 */
export function isSamePoolConfig(e, o) {
  return e.maxPoolNum === o.maxPoolNum && e.minPoolNum === o.minPoolNum && e.recycleInterval === o.recycleInterval && e.maxIdleTime === o.maxIdleTime && e.maxCoreSize === o.maxCoreSize && e.cacheCapacity === o.cacheCapacity && e.cacheTtl === o.cacheTtl;
}
