/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/info/common-sdk-info.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { SdkVersion } from '../../version/sdk-version';
import { DefaultSdk } from '../default-sdk';
import { SdkEtsComponent } from '../impl/sdk-ets-component';
import { SdkJsComponent } from '../impl/sdk-js-component';
import { SdkNativeComponent } from '../impl/sdk-native-component';
import { SdkPreviewerComponent } from '../impl/sdk-previewer-component';
import { SdkToolchainsComponent } from '../impl/sdk-toolchains-component';
import ApiMeta = ProjectBuildProfile.ApiMeta;
export declare abstract class CommonSdkInfo extends DefaultSdk {
    protected _log: OhosLogger;
    sdkDir: string;
    protected sdkVersion: ApiMeta;
    protected jsComponent: SdkJsComponent | undefined;
    protected etsComponent: SdkEtsComponent | undefined;
    protected nativeComponent: SdkNativeComponent | undefined;
    protected toolchainsComponent: SdkToolchainsComponent | undefined;
    protected previewerComponent: SdkPreviewerComponent | undefined;
    protected constructor(sdkVersion: ApiMeta, sdkDir: string);
    /**
     * 获取当前sdk中对应api 的toolchains组件的小版本号
     *
     * @return {string | undefined}
     */
    getToolchainsComponentVersion(): string | undefined;
    getModuleSchema(): string;
    getAppSchema(): string;
    getAppConfigurationSchema(): string;
    getStartWindowSchema(): string;
    getSystemThemeSchema(): string;
    getInsightIntentSchema(): string;
    getUtdSchema(): string;
    getFormSchema(): string;
    getPageSchema(): string;
    getRouterMapSchema(): string;
    getShareConfigSchema(): string;
    getShortcutsJsonSchema(): string;
    getAppStartupSchema(): string;
    getHspStartupSchema(): string;
    getHarStartupSchema(): string;
    getSysCapSchema(): string;
    getPacSchema(): string;
    getResIdDefinedSchema(): string;
    getSdkJsDir(): string;
    getSdkEtsDir(): string;
    getSdkVersion(): number;
    getSdkApi(): string;
    getHapTobin(): string;
    getReleaseType(): string;
    getSdkNativeDir(): string;
    getCmakeTool(): string;
    getNativeNinjaTool(): string;
    getNativeToolchain(): string;
    getNdkVersion(): SdkVersion;
    getSdkToolchainsDir(): string;
    getSdkLlvmStrip(): string;
    getSdkLlvmReadElf(): string;
    getRestool(): string;
    getSysCapTool(): string;
    getSysCapFileInEts(): string;
    getSysCapFileInJs(): string;
    getVerifySignConfigTool(): string;
    getJsArkVersion(compatibleSdkVersion: number, compatibleSdkVersionStage?: string): string;
    getRichSchema(): string;
    getLiteSchema(): string;
    getEtsArkVersion(compatibleSdkVersion: number, compatibleSdkVersionStage?: string): string;
    getEtsComponentVersion(): string;
    getEtsComponentReleaseType(): string;
    getArkVersion(compatibleSdkVersion: number, compatibleSdkVersionStage?: string): string;
    getPackageTool(): string;
    getUnPackageTool(): string;
    getBasePath(): string;
    getPreviewerCommonBinDir(): string;
    getPreviewerPermissionNames(): string[];
    getPreviewerUserGrant(): string[];
    getPreviewerManualSettings(): string[];
    allowSdkPermission(): boolean;
    getSignDir(): string;
    getJsLoader(): string;
    getEtsLoader(): string;
    requireUISyntax(): boolean;
    /**
     * 判断是否是preview构建，且sdk restool工具支持增量编译
     *
     * @param {boolean} isHarmonyOS 是否是HarmonyOS工程
     * @return {boolean} true是，false否
     */
    isPreviewCompileResourceIncrement(isHarmonyOS: boolean): boolean;
    /**
     * 判断SDK版本是否支持Ohpm工程
     * @param isHarmonyOS
     */
    supportOhpmProject(isHarmonyOS: boolean): boolean;
    /**
     * hap-sign-tool.jar是否支持API函数调用
     * @param isHarmonyOS
     */
    supportHapSignToolApiCall(isHarmonyOS: boolean): boolean;
    /**
     * 根据获取到的sdk-component名称创建不同的component实例
     *
     * @param component
     */
    protected initComponent(component: Component): void;
    private findNoAbcTool;
}
