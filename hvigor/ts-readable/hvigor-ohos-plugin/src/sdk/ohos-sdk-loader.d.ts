/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/ohos-sdk-loader.d.ts
 */
import { Component, PathAndApiVersion } from '@ohos/sdkmanager-common';
import { ProjectBuildProfile } from '../options/build/project-build-profile';
import ApiMeta = ProjectBuildProfile.ApiMeta;
export declare class OhosSdkLoader {
    private static container?;
    private sdkMap;
    private sdkInfoHandler?;
    private property;
    private constructor();
    static getInstance(): OhosSdkLoader;
    getOhosSdkComponents(sdkVersion: ApiMeta, components: string[]): Promise<Map<PathAndApiVersion, Component>>;
    private pickRequired;
    initHandler(): void;
    private validateCache;
}
