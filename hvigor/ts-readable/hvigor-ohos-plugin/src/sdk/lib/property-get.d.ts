/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/lib/property-get.d.ts
 */
export declare class Property {
    private _log;
    private _properties;
    private readonly _isHarmonyOS;
    constructor(isHarmonyOS?: boolean);
    /**
     * 读取当前命令行执行的路径下的local.properties的内容
     *
     * @returns {string} local.properties的内容
     */
    private _readFile;
    getProperty(key: string): string;
    getSdkDir(): string;
    getHosSdkDir(): string;
    getArkUIXSdkDir(): string;
    private validatePath;
    private _readPath;
    private _setPathIfNotExist;
    private _setProperty;
}
