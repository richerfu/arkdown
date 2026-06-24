/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/sdk-info-extension.d.ts
 */
import { ProjectBuildProfile } from '../options/build/project-build-profile';
import { ArkUIXSdkInfo } from './info/arkuix-sdk-info';
import ApiMeta = ProjectBuildProfile.ApiMeta;
/**
 * 扩展SDK
 *
 * @since 2023/05/18
 */
export declare class SdkInfoExtension {
    private readonly sdkVersion;
    private sdkInfo;
    private property;
    constructor(sdkVersion: ApiMeta);
    getArkUIXInfo(): ArkUIXSdkInfo;
    private getArkUIXSdkDir;
}
