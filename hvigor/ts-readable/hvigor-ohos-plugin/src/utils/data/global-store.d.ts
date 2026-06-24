/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/data/global-store.d.ts
 */
/**
 * 采用nodejs的require()模块的单例加载机制实现通用数据的共享
 */
declare class GlobalDataStore {
    private static instance;
    private store;
    constructor();
    set(key: string, value: any): GlobalDataStore;
    get(key: string): any;
    static getInstance(): GlobalDataStore;
}
declare const _default: GlobalDataStore;
export default _default;
