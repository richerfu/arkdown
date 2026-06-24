/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/sdk-loader.d.ts
 */
import { ProjectBuildProfile } from '../options/build/project-build-profile';
import { SdkInfo } from './sdk-info';
export declare class SdkLoader {
    isOhos: boolean;
    apiMeta: ProjectBuildProfile.ApiMeta;
    static loader?: SdkLoader;
    sdkInfo?: SdkInfo;
    private constructor();
    static getSdkInfo(apiMeta: ProjectBuildProfile.ApiMeta, isOhos?: boolean): SdkInfo;
    private static apiCompare;
}
