/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/context/impl/ohos-project-context-impl.d.ts
 */
import { SdkDetails } from '../../../external/sdk/sdk-details';
import { ProjectBuildProfile } from '../../../options/build/project-build-profile';
import { AppJson } from '../../../options/configure/app-json-options';
import { ProjectTaskService } from '../../../tasks/service/project-task-service';
import { AbstractProjectPlugin } from '../../common/abstract-project-plugin';
import { OhosProjectContext, OhpmDependencyInfo, Product } from '../plugin-context';
/**
 * Project级别的上下文信息
 */
export declare class OhosProjectContextImpl implements OhosProjectContext {
    private readonly projectPlugin;
    private currentProduct;
    private projectPath;
    private buildProductOutputPath;
    private buildRootPath;
    private taskService;
    private projectOhPackagePath;
    private logger;
    constructor(projectPlugin: AbstractProjectPlugin, taskService: ProjectTaskService);
    initCurrentProduct(): void;
    initBuildPath(): void;
    isDebugLineEnable(): boolean;
    isDebugLineEnableInternal(): boolean;
    private _isDebugLineEnable;
    getFastBuildApp(): boolean;
    getFastBuildAppInternal(): boolean;
    private _getFastBuildApp;
    getAppWithSignedPkg(): boolean;
    getAppWithSignedPkgInternal(): boolean;
    private _getAppWithSignedPkg;
    getAllModuleNameHash(): string;
    getAllModuleNameHashInternal(): string;
    private _getAllModuleNameHash;
    getOhpmDependencyInfo(): Record<string, OhpmDependencyInfo>;
    getOhpmDependencyInfoInternal(): Record<string, OhpmDependencyInfo>;
    private _getOhpmDependencyInfo;
    getOhpmRemoteHspDependencyInfo(isSigned?: boolean): Record<string, OhpmDependencyInfo>;
    getOhpmRemoteHspDependencyInfoInternal(isSigned?: boolean): Record<string, OhpmDependencyInfo>;
    private _getOhpmRemoteHspDependencyInfo;
    getProjectName(): string;
    getProjectNameInternal(): string;
    private _getProjectName;
    getBuildMode(): string;
    getBuildModeInternal(): string;
    private _getBuildMode;
    getProjectPath(): string;
    getProjectPathInternal(): string;
    private _getProjectPath;
    getCurrentProduct(): Product;
    getCurrentProductInternal(): Product;
    private _getCurrentProduct;
    getBuildProductOutputPath(): string;
    getBuildProductOutputPathInternal(): string;
    private _getBuildProductOutputPath;
    getBuildRootPath(): string;
    getBuildRootPathInternal(): string;
    private _getBuildRootPath;
    getAppJsonOpt(): AppJson.AppOptObj;
    getAppJsonOptInternal(): AppJson.AppOptObj;
    private _getAppJsonOpt;
    getBuildProfileOpt(): ProjectBuildProfile.ProjectProfileOpt;
    getBuildProfileOptInternal(): ProjectBuildProfile.ProjectProfileOpt;
    private _getBuildProfileOpt;
    setAppJsonOpt(appJsonOpt: AppJson.AppOptObj): void;
    setAppJsonOptInternal(appJsonOpt: AppJson.AppOptObj): void;
    private _setAppJsonOpt;
    setBuildProfileOpt(buildProfileOpt: ProjectBuildProfile.ProjectProfileOpt): void;
    setBuildProfileOptInternal(buildProfileOpt: ProjectBuildProfile.ProjectProfileOpt): void;
    private _setBuildProfileOpt;
    getDependenciesOpt(): any;
    getDependenciesOptInternal(): any;
    private _getDependenciesOpt;
    getDevDependenciesOpt(): any;
    getDevDependenciesOptInternal(): any;
    private _getDevDependenciesOpt;
    getDynamicDependenciesOpt(): any;
    getDynamicDependenciesOptInternal(): any;
    private _getDynamicDependenciesOpt;
    setDependenciesOpt(dependencies: any): void;
    setDependenciesOptInternal(dependencies: any): void;
    private _setDependenciesOpt;
    setDevDependenciesOpt(devDependencies: any): void;
    setDevDependenciesOptInternal(devDependencies: any): void;
    private _setDevDependenciesOpt;
    setDynamicDependenciesOpt(dynamicDependencies: any): void;
    setDynamicDependenciesOptInternal(dynamicDependencies: any): void;
    private _setDynamicDependenciesOpt;
    getOverrides(): any;
    getOverridesInternal(): any;
    private _getOverrides;
    setOverrides(overrides: any): void;
    setOverridesInternal(overrides: any): void;
    private _setOverrides;
    getSdkDetails(): SdkDetails;
    getSdkDetailsInternal(): SdkDetails;
    private _getSdkDetails;
}
