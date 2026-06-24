/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/make-project-pack-info.d.ts
 */
import { Project, TaskInputValue } from '@ohos/hvigor';
import { FileSet } from '@ohos/hvigor';
import { ProjectTaskService } from './service/project-task-service';
import { OhosAppTask } from './task/ohos-app-task';
/**
 * 生成Fa模型和Stage模型app级别的pack.info
 */
export declare class MakeProjectPackInfo extends OhosAppTask {
    private readonly _log;
    private readonly moduleSet;
    private readonly hapPackInfoPathMap;
    private readonly allRemoteHspPathMap;
    private readonly remoteHspModuleSet;
    private get alignDeviceTypes();
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    private _projectPackInfoObj;
    initTaskDepends(): void;
    private isAlignDeviceTypesContained;
    private normalizeDeviceTypes;
    private traverseNormalizeDeviceTypes;
    /**
     * 判断moduleModel对应的node模块是否存在一个Target需要打包到当前指定的product的app中
     * 判断的依据是在根项目的build-profile.json5中
     * 1.显示的配置了对应的target
     * 2.target的applyToProducts字段中包含了指定的product,默认如果没有配置applyToProducts则默认为applyTo default
     *
     * @param {ModuleModel} moduleModel
     * @returns {boolean}
     * @private
     */
    private isNeedPackageHap;
    constructor(project: Project, taskService: ProjectTaskService);
    /**
     * 执行打包之前的准备工作。
     *
     * @async
     * @function beforeAlwaysAction
     * @description 遍历所有产品数据，处理模块相关的信息，初始化远程HSP模块集合和路径映射。
     */
    beforeAlwaysAction(): Promise<void>;
    /**
     * 处理模块数据，解析模块路径信息并管理模块依赖。
     *
     * @param value - 包含模块目标数据的数组。
     * @param projectRemoteHspPath - 项目的远程HSP路径。
     */
    private handleModule;
    protected doTaskAction(): Promise<void>;
    /**
     * 合并该product包含的模块对应的hap的packInfo信息
     *
     * @param hapPackInfoPath hap的packInfo路径
     * @param moduleName 需要合并的模块名
     */
    private mergeHapPackInfo;
    /**
     * 合并该product包含的模块对应的remoteHsp的packInfo信息
     *
     * @param hspPackInfo remoteHsp的packInfo信息
     * @param moduleName 需要合并的模块名
     * @param hspFileName  远程hsp的文件名
     */
    private mergeRemoteHspPackInfo;
    private removeHspExtension;
}
