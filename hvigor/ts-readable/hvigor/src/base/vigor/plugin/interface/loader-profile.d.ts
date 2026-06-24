/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/plugin/interface/loader-profile.d.ts
 */
export interface LoaderProfile {
    system: Function;
    plugins: Function[];
    config: Record<string, any>;
}
/**
 * LoaderProfile对象
 */
export interface Config {
    getObject: (name: string) => any;
    setObject: (name: string, obj: object) => void;
}
export declare class CoreConfig implements Config {
    private logger;
    private config;
    constructor(config: any);
    getObject(name: string): any;
    setObject(name: string, obj: any): void;
}
