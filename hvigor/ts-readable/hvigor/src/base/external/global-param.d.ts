/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/global-param.d.ts
 */
export declare class GlobalParam {
    private _log;
    private static instance;
    private configuration;
    private _initialized;
    static getInstance(): GlobalParam;
    private constructor();
    private load;
    getConfig(key: string): any;
    setConfig(key: string, value: any): void;
    private check;
}
