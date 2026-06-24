/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/info/hos-sdk-info.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { CommonSdkInfo } from './common-sdk-info';
import ApiMeta = ProjectBuildProfile.ApiMeta;
export declare class HosSdkInfo extends CommonSdkInfo {
    private ohJavaComponent;
    private hosJavaComponent?;
    private hosToolchainsComponent?;
    private hmsCoreNativeComponent?;
    private hmsCoreEtsComponent?;
    private readonly hosComponents;
    private readonly hmsCoreComponents;
    protected localHosLibComponents: Map<string, Component>;
    protected localLibComponents: Map<string, Component>;
    constructor(requireComponents: string[], sdkVersion: ApiMeta, sdkDir: string);
    setup(): Promise<void>;
    hmsComponentInit(component: Component): void;
    getHmsArkDir(): string;
    getHmsNativeDir(): string;
    getHmsToolchainFile(): string;
    getHmsBiShengToolchainFile(): string;
    getSdkBinxoTool(): string;
    getRestool(): string;
    getHosResTool(): string;
    getModuleSchema(): string;
    getAppSchema(): string;
    getInsightIntentSchema(): string;
    getArkDataSchema(): string;
    getUtdSchema(): string;
    getRichSchema(): string;
    getLiteSchema(): string;
    getHapSignToolV2(): string;
    getHosModuleSchema(): string;
    getReleaseType(): string;
    getHosToolchainsComponentVersion(): string;
    getHosToolchainsDir(): string;
    getHosToolchainsLibDir(): string;
    getLibimageTranscoderShared(): string;
    getHosSignTool(): string;
    getPacSchema(): string;
    getSystemThemeSchema(): string;
}
