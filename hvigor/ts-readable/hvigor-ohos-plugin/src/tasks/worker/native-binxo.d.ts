/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/native-binxo.d.ts
 */
export interface BinxoOptions {
    excludeSoFromBinxo: string[];
    intermediatesLibs: string;
    sdkBinxoPath: string;
    binxoNativeLibs: string;
    moduleName: string;
    taskName: string;
    cacheFilePath: string;
    lastCache: string;
    collectAllLibs: boolean | undefined;
}
export declare function binxo(binxoOptions: BinxoOptions): Promise<void>;
