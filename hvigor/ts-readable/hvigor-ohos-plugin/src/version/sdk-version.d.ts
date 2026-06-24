/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/version/sdk-version.d.ts
 */
export declare class SdkVersion {
    private readonly _log;
    private readonly version;
    private readonly _major;
    private readonly _minor;
    private readonly _revision;
    private readonly _build;
    constructor(version: string);
    /**
     * 获取sdk component的4位小版本号
     *
     * @return {string}
     */
    getVersion(): string;
    getMajor(): number;
    getMinor(): number;
    getRevision(): number;
    getBuild(): number;
    /**
     * 判断当前版本号是否高于传入的版本
     *
     * @param {SdkVersion} sdkVersion
     * @returns {boolean}
     */
    isHigherThan(sdkVersion: SdkVersion): boolean;
    /**
     * 判断当前版本号是否等于传入的版本
     *
     * @param {SdkVersion} sdkVersion
     * @returns {boolean}
     */
    equals(sdkVersion: SdkVersion): boolean;
}
