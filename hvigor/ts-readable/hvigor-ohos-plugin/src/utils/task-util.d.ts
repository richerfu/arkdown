/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/task-util.d.ts
 */
import { CoreTask, Module, TaskCreation, TaskInfo } from '@ohos/hvigor';
import { ProjectModel } from '../model/project/project-model';
import { ProjectBuildProfile } from '../options/build/project-build-profile';
import { ModuleJson } from '../options/configure/module-json-options';
import { ModuleTargetData } from '../tasks/data/hap-task-target-data';
import { ModuleTaskService } from '../tasks/service/module-task-service';
import { ProjectTaskService } from '../tasks/service/project-task-service';
import { TargetTaskService } from '../tasks/service/target-task-service';
import { OhosLogger } from './log/ohos-logger';
export declare const resolveHookDependency: (name: string, target: string) => string;
export declare const dynamicResolveResourceTasks: (service: TargetTaskService, module: Module) => void;
export declare const warnCreateBuildProfileTask: (targetData: ModuleTargetData, targetService: TargetTaskService, log: OhosLogger) => void;
/**
 * 判断当前apiVersion是否高于limitApiVersion
 *
 * @param targetData
 * @param limitApiVersion
 */
export declare const limitMinApiVersion: (targetData: ModuleTargetData, limitApiVersion: number) => boolean;
/**
 * 重新加载依赖
 *
 * @param targetService
 *
 */
export declare const declareReloadDepends: (targetService: TargetTaskService) => string[];
/**
 * 获取当前buildModeName
 */
export declare const getBuildModeName: () => string;
/**
 * 判断是否需要编译卡片
 * @param formJsonPathArr
 * @param isFormExtensionModule
 */
export declare const shouldCompileArkTsWidget: (formJsonPathArr: string[], isFormExtensionModule: boolean) => boolean;
/**
 * 检测是否开了混淆，没开的话给出warning提示
 * @param targetService
 * @param _log
 */
export declare const doObfuscationCheck: (targetService: TargetTaskService, _log: OhosLogger) => void;
/**
 * 获取bundleType
 * @param product
 * @param projectModel
 */
export declare const getBundleType: (product: ProjectBuildProfile.ProductBuildOpt, projectModel: ProjectModel) => string;
/**
 * 判断是否为元服务工程
 * @param product
 * @param projectModel
 */
export declare const isAtomicServiceProject: (product: ProjectBuildProfile.ProductBuildOpt, projectModel: ProjectModel) => boolean;
/**
 * 判断启动框架配置文件中appPreloadHintStartupTasks每一项srcEntry的so文件是否在中间产物libs目录下存在
 * 有一种特殊情况：在processLibs任务中若har模块bundledDependencies为true时不收集其依赖字节码har的so，所以如果当前har模块bundledDependencies为true，
 * 则其所依赖的远程字节码har的so跳过检测
 * @param appStartupPath
 * @param baseNativeLibsPath
 * @param _log
 * @param moduleName
 * @param isBundledDependencies
 * @private
 */
export declare const checkAppPreloadHintStartupTasksSrcEntry: (appStartupPath: string, baseNativeLibsPath: string, _log: OhosLogger, moduleName: string, isBundledDependencies?: boolean) => void;
/**
 * Checks if fast build app is enabled for the given service.
 *
 * @param {ModuleTaskService} service - The service to check.
 * @returns {boolean} - Returns true if fast build app is enabled, otherwise false.
 */
export declare const isFastBuildApp: (service: ModuleTaskService | ProjectTaskService) => boolean;
/**
 * 判断是否为HomeAbility，判断条件：
 * entities包含entity.system.home && (actions中包含action.system.home || actions包含ohos.want.action.home)
 * @param ability
 */
export declare const isHomeAbility: (ability: ModuleJson.AbilityObj) => boolean;
/**
 * 从 CoreTask 实例中提取任务信息。
 *
 * @param task - 要提取信息的 CoreTask 实例。
 * @returns TaskInfo对象。
 */
export declare const getTaskInfoFromTask: (task: CoreTask) => TaskInfo;
/**
 * 从 TaskCreation 实例中提取任务信息。
 *
 * @param taskCA - 要提取信息的 TaskCreation 实例。
 * @returns TaskInfo对象。
 */
export declare const getTaskInfoFromTaskCA: (taskCA: TaskCreation) => TaskInfo;
