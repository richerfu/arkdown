/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/build/build-opt.d.ts
 */
import { Obfuscation } from '@ohos/hvigor-arkts-compose';
import { AotCompileModeEnum } from '../../enum/aot-compile-mode-enum';
import { CompileModeEnum } from '../../enum/compile-mode-enum';
import { Named, Option, RequiredNamed } from '../options';
export interface BuildOptionWithName extends Named, BuildOpt {
    copyFrom?: string;
}
export interface BuildOpt extends ProjectBuildOpt, ModuleBuildOpt {
    removePermissions?: RequiredNamed[];
}
interface ProjectBuildOpt {
    packOptions?: PackOpt;
    preloadSystemSo?: boolean;
}
interface ModuleBuildOpt extends HapModuleBuildOpt, HarModuleBuildOpt {
}
interface HarModuleBuildOpt extends CommonModuleBuildOpt {
    artifactType?: string;
    packingOptions?: PackingOptions;
}
interface HapModuleBuildOpt extends CommonModuleBuildOpt {
    debuggable?: boolean;
    sourceOption?: SourceOpt;
    compileMode?: CompileModeEnum;
    aotCompileMode?: AotCompileModeEnum;
    apPath?: string;
    generateSharedTgz?: boolean;
}
interface CommonModuleBuildOpt extends Option {
    externalNativeOptions?: ExternalNativeOpt;
    napiLibFilterOption?: NapiLibFilterOpt;
    arkOptions?: ArkOptions;
    nativeLib?: NativeLib;
    strictMode?: StrictMode;
    cangjieOptions?: CangjieOpt;
    resOptions?: {
        compression?: RestoolCompressionConfig;
        copyCodeResource?: CopyCodeResource;
        resCompileThreads?: number;
        ignoreResourcePattern?: string[];
        qualifiersConfig?: QualifiersConfig;
        includeAppScopeRes?: boolean;
        excludeHarRes?: string[];
        idDefinedFilePath?: string;
        [key: string]: any;
    };
    nativeCompiler?: string;
}
export interface StrictMode extends Option {
    noExternalImportByPath?: boolean;
    useNormalizedOHMUrl?: boolean;
    caseSensitiveCheck?: boolean;
    strictCheckerOnly?: boolean;
    enableStrictCheckOHModule?: boolean;
    disableSendableCheckRules?: string[];
}
export interface NativeLib extends Option {
    debugSymbol?: DebugSymbol;
    filter?: NapiLibFilterOpt;
    headerPath?: string | string[];
    collectAllLibs?: boolean;
    excludeSoFromBinXO?: string[];
    librariesInfo?: librariesInfo[];
    excludeFromHar?: boolean;
    excludeSoFromInterfaceHar?: boolean;
}
export interface DebugSymbol extends Option {
    strip?: boolean;
    exclude?: string[];
}
export interface librariesInfo extends Option {
    name: string;
    linkLibraries?: string[];
}
export interface NapiLibFilterOpt extends Option {
    excludes?: string[];
    pickFirsts?: string[];
    pickLasts?: string[];
    enableOverride?: boolean;
    select?: PkgSelection[];
}
export interface PkgSelection extends Option {
    package: string;
    version?: string;
    include?: string[];
    exclude?: string[];
    includePattern?: string[];
    excludePattern?: string[];
}
export interface ExternalNativeOpt extends Option {
    path?: string;
    arguments?: string | string[];
    abiFilters?: string[];
    cppFlags?: string;
    cFlags?: string;
    targets?: string[];
}
export interface TscConfig {
    targetESVersion?: string;
    maxFlowDepth?: number;
}
export interface ExpandImportPath {
    enable: boolean;
    exclude?: string[];
}
export interface AutoLazyFilter {
    exclude?: string[];
    include?: string[];
}
export interface ArkOptions {
    aotCompileMode?: AotCompileModeEnum;
    apPath?: string;
    hostPGO?: boolean;
    types?: string[];
    compilePluginFile?: string;
    obfuscation?: Obfuscation;
    buildProfileFields?: object;
    runtimeOnly?: RuntimeOnlyObj;
    integratedHsp?: boolean;
    tscConfig?: TscConfig;
    branchElimination?: boolean;
    transformLib?: string | undefined;
    byteCodeHar?: boolean;
    bundledDependencies?: boolean;
    autoLazyImport?: boolean;
    reExportCheckMode?: string;
    packSourceMap?: boolean;
    skipOhModulesLint?: boolean;
    expandImportPath?: ExpandImportPath;
    autoLazyFilter?: AutoLazyFilter;
}
export interface PackingOptions {
    asset?: PackingOptAsset;
    customizedOptions?: CustomizedOptions;
}
export interface CustomizedOptions {
    basePackage?: string;
}
export interface PackingOptAsset {
    include?: string[];
    exclude?: string[];
}
export interface CangjieOpt extends Option {
    path?: string;
}
export interface SourceOpt extends Option {
    workers?: string[];
}
export interface RuntimeOnlyObj extends Option {
    sources?: string[];
    packages?: string[];
    excludePackages?: string[];
}
export interface PackOpt extends RequiredNamed {
    buildAppSkipSignHap?: boolean;
    fastBuildApp?: boolean;
    enableSourceCodeCheck?: boolean;
    deduplicateHar?: boolean;
    appWithSignedPkg?: boolean;
    enableIncrementalSoCompress?: boolean;
}
export interface RestoolCompressionFilterItem {
    method: {
        type: 'astc' | 'sut';
        blocks: '4x4';
    };
    files?: {
        path?: string[];
        size?: (number | string)[][];
        resolution?: {
            width: number;
            height: number;
        }[][];
    };
    exclude?: {
        path?: string[];
        size?: (number | string)[][];
        resolution?: {
            width: number;
            height: number;
        }[][];
    };
}
export interface RestoolCompressionConfig {
    media?: {
        enable?: boolean;
    };
    sizeLimit?: {
        ratio?: number;
    };
    filters?: RestoolCompressionFilterItem[];
}
export interface CopyCodeResource extends Option {
    enable?: boolean;
    excludes?: string[];
    includes?: string[];
}
export interface QualifiersConfig {
    'Mcc&Mnc'?: string[];
    Locale?: string[];
    Orientation?: string[];
    Device?: string[];
    ColorMode?: string[];
    Density?: string[];
}
export {};
