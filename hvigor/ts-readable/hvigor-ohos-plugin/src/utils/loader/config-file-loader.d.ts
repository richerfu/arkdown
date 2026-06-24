/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/config-file-loader.d.ts
 */
declare class ConfigFileLoader {
    configFileJsonMap: Map<string, any>;
    clean(): void;
    getConfigFileJson(filePath: string): any;
    setConfigFileJson(filePath: string, fileJsonObj: any): void;
}
export declare const configFileLoader: ConfigFileLoader;
export {};
