/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/info/ohos-sdk-info.d.ts
 */
import { Component, PathAndApiVersion } from '@ohos/sdkmanager-common';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { CommonSdkInfo } from './common-sdk-info';
import ApiMeta = ProjectBuildProfile.ApiMeta;
export declare class OhosSdkInfo extends CommonSdkInfo {
    private readonly ohComponents;
    private ohosLocalComponents;
    constructor(requireComponents: string[], sdkVersion: ApiMeta, sdkDir: string);
    setOhosMetaCompileSdkVersion(): void;
    setup(): Promise<void>;
    contains(pathAndApi: PathAndApiVersion, all: Map<PathAndApiVersion, Component>): Component | undefined;
    getLibimageTranscoderShared(): string;
    getHosToolchainsLibDir(): string;
    getSystemThemeSchema(): string;
    getArkDataSchema(): string;
}
