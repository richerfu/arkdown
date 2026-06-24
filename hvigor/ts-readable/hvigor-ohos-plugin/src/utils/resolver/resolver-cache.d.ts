/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/resolver/resolver-cache.d.ts
 */
export declare class ResolverCache {
    private static cacheMap;
    static has(key: string): boolean;
    static get(key: string): string | undefined;
    static set(key: string, val: string): Map<string, string>;
    static clear(): void;
}
