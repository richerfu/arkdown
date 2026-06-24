/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/context/plugin-context.d.ts
 */
import { TransformAbcCallback } from '../../external/callback/transform-abc';
import { PathDetails } from '../../external/path/path-details';
import { SdkDetails } from '../../external/sdk/sdk-details';
import { BuildOption } from '../../options/build/hap-ohos-config';
import { ModuleBuildProfile } from '../../options/build/module-build-profile';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { AppJson } from '../../options/configure/app-json-options';
import { ModuleJson } from '../../options/configure/module-json-options';
/**
 * Project级别的Plugins上下文信息
 */
export interface OhosProjectContext {
    getProjectName: () => string;
    getProjectPath: () => string;
    getBuildRootPath: () => string;
    getBuildProductOutputPath: () => string;
    getCurrentProduct: () => Product;
    getBuildMode: () => string;
    getOhpmDependencyInfo: () => Record<string, OhpmDependencyInfo> | object;
    getOhpmRemoteHspDependencyInfo: (isSigned: boolean | undefined) => Record<string, OhpmDependencyInfo>;
    getBuildProfileOpt: () => ProjectBuildProfile.ProjectProfileOpt;
    setBuildProfileOpt: (buildProfileOpt: ProjectBuildProfile.ProjectProfileOpt) => void;
    getAppJsonOpt: () => AppJson.AppOptObj;
    setAppJsonOpt: (appJsonOpt: AppJson.AppOptObj) => void;
    getDependenciesOpt: () => any;
    getDevDependenciesOpt: () => any;
    getDynamicDependenciesOpt: () => any;
    setDependenciesOpt: (dependencies: any) => void;
    setDevDependenciesOpt: (devDependencies: any) => void;
    setDynamicDependenciesOpt: (dynamicDependencies: any) => void;
    getOverrides: () => any;
    setOverrides: (overrides: any) => void;
    getSdkDetails: () => SdkDetails;
    isDebugLineEnable: () => boolean;
    getAllModuleNameHash: () => string;
    getFastBuildApp: () => boolean;
    getAppWithSignedPkg: () => boolean;
}
export interface Product {
    getProductName: () => string;
    getBundleType: () => string;
    getBundleName: () => string;
    getOutputArtifactName: () => string | undefined;
}
/**
 * Module级别的Plugin上下文信息
 */
export interface OhosModuleContext {
    getModuleName: () => string;
    getModulePath: () => string;
    getModuleType: () => string;
    getBuildProductRootPath: () => string;
    targets: (callback: (target: Target) => void) => void;
    getOhpmDependencyInfo: () => Record<string, OhpmDependencyInfo>;
    getOhpmRemoteHspDependencyInfo: (isSigned: boolean) => Record<string, OhpmDependencyInfo>;
    getBuildProfileOpt: () => ModuleBuildProfile.ModuleBuildOpt;
    setBuildProfileOpt: (buildProfileOpt: ModuleBuildProfile.ModuleBuildOpt) => void;
    getModuleJsonOpt: () => ModuleJson.ModuleOptObj;
    setModuleJsonOpt: (moduleJsonOpt: ModuleJson.ModuleOptObj) => void;
    getDependenciesOpt: () => any;
    getDevDependenciesOpt: () => any;
    getDynamicDependenciesOpt: () => any;
    setDependenciesOpt: (dependencies: any) => void;
    setDevDependenciesOpt: (devDependencies: any) => void;
    setDynamicDependenciesOpt: (dynamicDependencies: any) => void;
    getVersion: () => string;
    setVersion: (version: string) => void;
    loadCompilePlugin: (path: string) => void;
    getBuildMode: () => string;
    getTargetArtifactNames: () => Map<string, string>;
    getTargetArtifactName: (targetName: string) => string | undefined;
    transformAbc: (fn: TransformAbcCallback, targetName?: string) => void;
}
export interface OhpmDependencyInfo {
    name: string;
    version: string;
    dependencies: Record<string, string>;
    packagePath: string;
    remoteHspPath?: string;
    signedRemoteHspPath?: string;
}
export interface DependRemoteHspInfo extends OhpmDependencyInfo {
    soPath?: string;
}
export interface Target {
    getCurrentProduct: () => Product;
    getBuildTargetOutputPath: () => string;
    getTargetName: () => string;
    getBuildOption: () => BuildOption;
    getModulePathDetails: () => PathDetails;
}
export type OhosAppContext = OhosProjectContext;
export type OhosHapContext = OhosModuleContext;
export type OhosHspContext = OhosModuleContext;
export type OhosHarContext = OhosModuleContext;
