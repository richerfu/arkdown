/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/external/sdk/sdk-details.d.ts
 */
export interface SdkDetails {
    isOhos(): boolean;
    getSdkDir(): string;
    getSdkVersion(): number;
    getEtsComponentVersion(): string;
    getEtsComponentReleaseType(): string;
}
