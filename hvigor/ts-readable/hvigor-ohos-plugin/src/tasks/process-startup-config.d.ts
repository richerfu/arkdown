/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-startup-config.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
export declare class ProcessStartupConfig extends OhosHapTask {
    private readonly _log;
    private startupMergedOptions;
    protected readonly targetJsonPath: string;
    private readonly hspNeedDeduplicateHarInfo;
    private allHspDepTargets;
    constructor(targetService: TargetTaskService);
    private get useNormalizedOHMUrl();
    private get shouldCollectPreloadSystemSo();
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): void;
    initTaskDepends(): void;
    /**
     * appStartup.json文件schema校验
     * @private
     */
    private appStartupValidate;
    /**
     * startup配置文件业务校验
     * startupTasks字段：srcEntry真实存在，且ets/ts/js后缀，name唯一
     * appPreloadHintStartupTask字段：srcEntry后缀为so，name唯一
     * 每个模块（本模块和依赖的har模块）单独判断：srcEntry真实存在，后缀名合规；再合并，合并后判断name是否唯一
     *
     * @private
     */
    private checkAppStartupConfig;
    /**
     * 校验如"routerMap":"$profile:xxx"等资源引用方式的配置,若对应文件不存在则报错
     * @param configField
     * @private
     */
    private checkResReferenceConfig;
    /**
     * 处理本地依赖har
     * @param harDependency
     * @param pkgContextInfos
     * @private
     */
    private processLocalHarDependency;
    /**
     * 得到依赖模块本地har的启动框架文件名和启动框架配置路径
     *
     * @param harDependency
     * @private
     */
    private getLocalHarDepStartupConfig;
    /**
     * 得到本地依赖har的资源下的启动框架配置文件路径（适配多target场景）
     *
     * @private
     */
    private getLocalHarDepStartupFile;
    /**
     * 收集har包（不包括字节码har）的preload.json，将其合并到startupMergedOptions，后续用于落盘
     * @param dependency
     * @private
     */
    private addDependencyPreloadSystemSoJson;
    /**
     * 处理远程依赖的har的startup，远程har基础校验不用做，在打包的时候做过了
     * @private
     * @param remoteDependency
     * @param pkgContextInfos
     */
    private processRemoteHarDependency;
    /**
     * 获取远程依赖har（产物态）的启动框架配置文件（适配多target场景）
     * @param harDependency
     * @param appStartupProfileName
     * @private
     */
    private getRemoteHarDepStartupFile;
    /**
     * 处理本模块的startup配置文件校验
     * @private
     */
    private processModuleStartupConfig;
    /**
     * 1.srcEntry校验
     * 2.configEntry校验
     * srcEntry不能是绝对路径；必须是本模块文件；文件必须真实存在;指向文件必须以ets,ts,js后缀结尾
     * @param appStartupPath
     * @param srcEntryPath
     * @param moduleModelPath
     * @param moduleName
     * @private
     */
    private checkStartupPath;
    /**
     * appPreloadHintStartupTask中srcEntry值的业务校验
     *  1 校验srcEntry的值是否是so后缀结尾
     *  2 校验srcEntry的值在信息语义表pkgContextInfo.json中是否存在
     *
     * @param appStartupPath
     * @param appPreloadHintStartupTaskPath
     * @param moduleName
     * @param pkgContextInfos
     * @private
     */
    private checkAppPreloadHintStartupTaskPath;
    /**
     * 校验启动框架配置文件中startupTasks和appPreloadHintStartupTasks任务对象集合中对象名称唯一
     * @private
     * @param startupTaskObjList
     * @param startTaskType
     */
    private checkStartupTaskName;
    /**
     * 编译hap时把hsp搜集到的去重startup文件作为输入
     * 要搜集直接依赖和间接依赖的所有本地hsp
     */
    private processHarDeduplicateInputFiles;
    /**
     * 处理har包去重逻辑
     */
    private processHarDeduplicate;
    /**
     * 编译hsp时搜集去重har包里startUp配置并存储到临时中间文件中
     */
    private collectDuplicateHarStartup;
    /**
     * 编译hap时合并去重har包的startUp配置到hap中（如果没有的话）
     */
    private addDuplicateHarStartups;
}
