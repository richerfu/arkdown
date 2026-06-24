/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/native-cmake-cache.d.ts
 */
export interface CmakeCache {
    entries: CacheEntry[];
}
export interface CacheEntry {
    name: string;
    value: string;
    type: string;
    properties?: CacheEntryProperty[];
}
export interface CacheEntryProperty {
    name: string;
    value: string;
}
