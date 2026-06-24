/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/context/impl/ohos-module-context-impl.d.ts
 */
import { TransformAbcCallback } from '../../../external/callback/transform-abc';
import { ModuleBuildProfile } from '../../../options/build/module-build-profile';
import { ModuleJson } from '../../../options/configure/module-json-options';
import { AbstractModulePlugin } from '../../common/abstract-module-plugin';
import { OhosModuleContext, OhpmDependencyInfo, Target } from '../plugin-context';
/**
 * Module级别的上下文信息
 */
export declare class OhosModuleContextImpl implements OhosModuleContext {
    private readonly logger;
    private readonly modulePlugin;
    private readonly targetArr;
    private readonly taskService;
    private readonly moduleOhPackagePath;
    private readonly targetArtifactNames;
    constructor(modulePlugin: AbstractModulePlugin);
    targets(callback: (target: Target) => void): void;
    targetsInternal(callback: (target: Target) => void): void;
    private _targets;
    getModuleName(): string;
    getModuleNameInternal(): string;
    private _getModuleName;
    getTargetArtifactNames(): Map<string, string>;
    getTargetArtifactNamesInternal(): Map<string, string>;
    private _getTargetArtifactNames;
    getTargetArtifactName(targetName: string): string | undefined;
    getTargetArtifactNameInternal(targetName: string): string | undefined;
    private _getTargetArtifactName;
    getModulePath(): string;
    getModulePathInternal(): string;
    private _getModulePath;
    getModuleType(): string;
    getModuleTypeInternal(): string;
    private _getModuleType;
    getBuildProductRootPath(): string;
    getBuildProductRootPathInternal(): string;
    private _getBuildProductRootPath;
    getOhpmDependencyInfo(): Record<string, OhpmDependencyInfo>;
    getOhpmDependencyInfoInternal(): Record<string, OhpmDependencyInfo>;
    private _getOhpmDependencyInfo;
    getOhpmRemoteHspDependencyInfo(isSigned?: boolean): Record<string, OhpmDependencyInfo>;
    getOhpmRemoteHspDependencyInfoInternal(isSigned?: boolean): Record<string, OhpmDependencyInfo>;
    private _getOhpmRemoteHspDependencyInfo;
    getBuildProfileOpt(): ModuleBuildProfile.ModuleBuildOpt;
    getBuildProfileOptInternal(): ModuleBuildProfile.ModuleBuildOpt;
    private _getBuildProfileOpt;
    getModuleJsonOpt(): ModuleJson.ModuleOptObj;
    getModuleJsonOptInternal(): ModuleJson.ModuleOptObj;
    private _getModuleJsonOpt;
    setBuildProfileOpt(buildProfileOpt: ModuleBuildProfile.ModuleBuildOpt): void;
    setBuildProfileOptInternal(buildProfileOpt: ModuleBuildProfile.ModuleBuildOpt): void;
    private _setBuildProfileOpt;
    setModuleJsonOpt(moduleJsonOpt: ModuleJson.ModuleOptObj): void;
    setModuleJsonOptInternal(moduleJsonOpt: ModuleJson.ModuleOptObj): void;
    private _setModuleJsonOpt;
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
    getVersion(): string;
    getVersionInternal(): string;
    private _getVersion;
    setVersion(version: string): void;
    setVersionInternal(version: string): void;
    private _setVersion;
    loadCompilePlugin(pluginPath: string): void;
    loadCompilePluginInternal(pluginPath: string): void;
    private _loadCompilePlugin;
    getBuildMode(): string;
    getBuildModeInternal(): string;
    private _getBuildMode;
    transformAbc(fn: TransformAbcCallback, targetName?: string): void;
    transformAbcInternal(fn: TransformAbcCallback, targetName?: string): void;
    private _transformAbc;
}
