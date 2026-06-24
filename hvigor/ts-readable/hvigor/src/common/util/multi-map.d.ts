/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/multi-map.d.ts
 */
export declare class MultiMap {
    private _nMap;
    put(key: string, value?: string): void;
    get(key: string): Set<string>;
    keys(): string[];
    removeAll(key: string): Set<string>;
    remove(key: string, value: string): void;
    clear(): void;
}
