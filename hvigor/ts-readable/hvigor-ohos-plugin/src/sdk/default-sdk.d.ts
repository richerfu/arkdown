/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/default-sdk.d.ts
 */
import { SdkInfo } from './sdk-info';
export declare class DefaultSdk implements SdkInfo {
    hasRollUpPluginInEtsLoader: boolean;
    hasRollUpPluginInJsLoader: boolean;
    isOhos: boolean;
    sdkDir: string;
    setup(): Promise<void>;
    checkAotFixedSdkVersion(): boolean;
    getAppSchema(): string;
    getInsightIntentSchema(): string;
    getArkDataSchema(): string;
    getUtdSchema(): string;
    getArkUIXSdkDir(): string;
    getArkVersion(compatibleSdkVersion: number, compatibleSdkVersionStage?: string): string;
    getCmakeTool(): string;
    getEtsComponentReleaseType(): string;
    getEtsComponentVersion(): string;
    getHosToolchainsDir(): string;
    getEtsLoader(): string;
    getFormSchema(): string;
    getHapSignToolV2(): string;
    getHapTobin(): string;
    getHmsArkDir(): string;
    getHmsNativeDir(): string;
    getHmsToolchainFile(): string;
    getHmsBiShengToolchainFile(): string;
    getHosModuleSchema(): string;
    getHosResTool(): string;
    getJsLoader(): string;
    getLiteSchema(): string;
    getModuleSchema(): string;
    getNativeNinjaTool(): string;
    getNativeToolchain(): string;
    getPackageTool(): string;
    getUnPackageTool(): string;
    getPageSchema(): string;
    getStartWindowSchema(): string;
    getSystemThemeSchema(): string;
    getRouterMapSchema(): string;
    getShareConfigSchema(): string;
    getShortcutsJsonSchema(): string;
    getAppStartupSchema(): string;
    getHspStartupSchema(): string;
    getHarStartupSchema(): string;
    getSysCapSchema(): string;
    getPacSchema(): string;
    getResIdDefinedSchema(): string;
    getReleaseType(): string;
    getRestool(): string;
    getRichSchema(): string;
    getSdkEtsDir(): string;
    getSdkJsDir(): string;
    getSdkNativeDir(): string;
    getSdkToolchainsDir(): string;
    getSdkBinxoTool(): string;
    getSdkLlvmStrip(): string;
    getSdkLlvmReadElf(): string;
    getSdkVersion(): number;
    getHosToolchainsLibDir(): string;
    getLibimageTranscoderShared(): string;
    getPreviewerCommonBinDir(): string;
    getPreviewerPermissionNames(): string[];
    getPreviewerUserGrant(): string[];
    getPreviewerManualSettings(): string[];
    allowSdkPermission(): boolean;
    getSignDir(): string;
    getHosSignTool(): string;
    getSysCapFileInEts(): string;
    getSysCapFileInJs(): string;
    getSysCapTool(): string;
    getToolchainsComponentVersion(): string | undefined;
    getHosToolchainsComponentVersion(): string | undefined;
    getVerifySignConfigTool(): string;
    isPreviewCompileResourceIncrement(isHarmonyOS: boolean): any;
    requireUISyntax(): boolean;
    supportOhpmProject(isHarmonyOS: boolean): any;
    supportHapSignToolApiCall(isHarmonyOS: boolean): boolean;
    getSdkApi(): string;
    getBasePath(): string;
    getAppConfigurationSchema(): string;
}
