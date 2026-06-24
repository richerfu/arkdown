/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/ohos-har-task.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { RuntimeOnlyObj } from '../../options/build/build-opt';
import { ModuleBuildProfile } from '../../options/build/module-build-profile';
import { HarExtendInfo } from '../har/har-extend-info';
import { TargetTaskService } from '../service/target-task-service';
import { OhosModuleTask } from './ohos-module-task';
export declare const DEFAULT_MAIN_FILED_FILE_SUFFIX: string[];
export interface PackageJsonOpt {
    name: string;
    version: string;
    ohos: {
        org: string;
        artifactType: string;
    };
    main: string;
    types?: string;
    parameterFile?: string;
    dependencies?: object;
    properties?: Record<string, unknown>;
}
export interface OhPackageJsonOpt {
    name: string;
    version: string;
    artifactType: string;
    main: string;
    types: string;
    'oh-exports'?: string[] | undefined;
    metadata?: {
        workers?: string[];
        declarationEntry?: string[];
        runtimeOnly?: RuntimeOnlyObj;
        resource?: ModuleBuildProfile.ResourceBuildOpt;
        sourceRoot?: string[];
    };
    parameterFile?: string;
    dependencies?: object;
    properties?: Record<string, unknown>;
}
/**
 * OpenHarmony har的base task
 *
 * @since 2022/1/5
 */
export declare abstract class OhosHarTask extends OhosModuleTask {
    readonly _harExtendInfo: HarExtendInfo;
    protected readonly modulePackageJsonPath: string;
    protected constructor(targetService: TargetTaskService, taskDetails: TaskDetails);
    private getModulePackageJsonPath;
    /**
     * 根据har模块下的package.json中main字段的配置,获取有效的main字段用于最终生成package.json
     * 1.配置了则使用配置的，并校验是否合法
     * 2.没有配置或者配置为空则按顺序检索模块根目录下存在哪种后缀的文件
     * 3.在2的基础上没有检索到,则默认使用index.ets为入口
     *
     * @returns {string}
     * @protected
     */
    protected getPackageJsonValidMainField(): string;
    private findValidMainField;
}
