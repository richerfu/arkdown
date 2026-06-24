/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/cache.d.ts
 */
export declare const caseErrorSet: Set<string>;
export declare const isDirCached: {
    (param: any, finish: Function, options?: Record<any, any>): Promise<any>;
    clear(): void;
};
export declare const isFileCached: {
    (param: any, finish: Function, options?: Record<any, any>): Promise<any>;
    clear(): void;
};
export declare const readCachedFile: {
    (param: any, finish: Function, options?: Record<any, any>): Promise<any>;
    clear(): void;
};
export declare function clearStepAddressingCache(): void;
