/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/dependency-util.d.ts
 */
import { ProjectModel } from '../model/project/project-model';
import { Dependency, DependencyEnum } from '../project/dependency/core/dependency-interface';
import { ModuleTaskService } from '../tasks/service/module-task-service';
import { TargetTaskService } from '../tasks/service/target-task-service';
import { PackageJson } from '../model/package-json';
export type OhosPackageJson = PackageJson & {
    name?: string;
    ohos?: {
        org: string;
        artifactType: string;
    };
    artifactType?: string;
};
export declare function getJson5Obj(filePath: string, isClone?: boolean): any;
/**
 * 最小化hsp中的依赖，只保留hsp中的hsp相关依赖并删除devDependencies中的依赖
 *
 * @param packageJsonObj  oh-package.json5文件对象
 * @param service  ModuleTaskService
 * @param packageManagerPath  oh-package.json5的路径
 */
export declare function minimizeHspDependencies(packageJsonObj: any, service: ModuleTaskService, packageManagerPath: string): void;
/**
 * 打包hsp时dependencies依赖会排除hsp以外的其他依赖
 *
 * @param dependencyEnum 依赖类型
 * @param service  ModuleTaskService
 * @param packageManagerPath  oh-package.json5的路径
 * @private
 */
export declare function getDependenciesWithoutHar(dependencyEnum: DependencyEnum, service: ModuleTaskService, packageManagerPath: string): any;
/**
 * 获取本地依赖har模块的当前target资源路径集合
 * @private
 */
export declare function getLocalDepHarTargetSources(targetTaskService: TargetTaskService, harDependency: Dependency, projectModule: ProjectModel): string[];
/**
 * 获取远程har（产物态）模块的资源路径集合，产物态和源码态有一点区别：若在打包时target的resource/directories中配置了资源路径，则只会把配置的
 * 资源路径达到har包里，也就是说这种情况下产物har里不存在默认资源路径；但是当resource/directories是空时，会将默认资源路径达到包里
 * @param harDependency
 */
export declare function getRemoteDepHarTargetSources(harDependency: Dependency): string[];
/**
 * 获取本地依赖的路由表数据
 * @param projectModel
 * @param targetTaskService
 * @param dependency
 */
export declare function getLocalDependencyRouterMap(projectModel: ProjectModel, targetTaskService: TargetTaskService, dependency: Dependency): import("../options/configure/router-map-options").RouterMapOptions.RouterMapObj[] | undefined;
/**
 * 获取本地依赖的共享配置数据
 * @param projectModel
 * @param targetTaskService
 * @param dependency
 */
export declare function getLocalDependencyShareConfig(projectModel: ProjectModel, targetTaskService: TargetTaskService, dependency: Dependency): import("../options/configure/share-config-options").ShareConfigOptions.ShareConfigObj[] | undefined;
/**
 * 基于target差异化获取依赖的buildOptions
 * @param dependency
 * @param moduleName2TargetName
 */
export declare function getLocalDependencyBuildOptions(dependency: Dependency, moduleName2TargetName: Map<string, string>): import("../options/build/build-opt").BuildOpt;
