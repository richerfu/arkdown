/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-syscap-transform.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { ModulePathInfoIml } from '../../common/iml/module-path-info-iml';
import { JsonProfile } from '../../model/json-profile';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * Syscap Transform task
 *
 * @since 2022/09/8
 */
export declare abstract class AbstractSyscapTransform extends OhosHapTask {
    private _log;
    protected readonly sourceRoot: string;
    readonly pathInfo: ModulePathInfoIml;
    protected readonly sysCapJsonPath: string;
    protected readonly rpcidScPath: string;
    protected get sysCapTool(): string;
    protected get sysCapPath(): string;
    protected get deviceTypes(): string[];
    protected constructor(taskService: TargetTaskService, taskName: TaskDetails);
    initTaskDepends(): void;
    protected doTaskAction(): Promise<void>;
    protected abstract getJsonProfileByModel(): JsonProfile;
    /**
     * 获取general中定义的设备类型集合其对应设备定义的syscap能力集的交集
     * @param deviceTypesList
     * @protected
     */
    protected getIntersectedGeneralSysCapSet(deviceTypesList: string[]): Set<string>;
    /**
     * 根据设备类型从sdk中获取其对应的syscap能力集
     * 如果设备无syscap定义json文件则返回undefined
     * @param deviceType
     * @param deviceDefinePath
     * @protected
     */
    protected getSysCapSetFromSdk(deviceType: string, deviceDefinePath: string): Set<string> | undefined;
    /**
     * 获得syscap.json中custom定义的N设备类型其对应的syscap能力集交集
     * @param customSysCapList NDeviceType中SysCap
     */
    protected getIntersectedCustomSysCapSet(customSysCapList: object[] | undefined): Set<string>;
    /**
     * 根据production配置对sysCap交集进行增删处理
     * @param productionObj
     * @param intersectedSysCapSet
     * @protected
     */
    protected processProductionSysCap(productionObj: ProductionObj, intersectedSysCapSet: Set<string>): void;
    /**
     * 处理custom字段异常场景
     * 1.custom字段不存在/custom字段存在但数组为空没有对象则不处理custom
     * 2.custom存在 数组不空但是对象为空则按空集报错处理
     * @param customSysCapList
     * @private
     */
    private checkCustomSysCapList;
}
/**
 * syscap.json对象配置
 */
export interface SyscapJsonObj {
    devices: SyscapDeviceObj;
    production: ProductionObj;
    development: DevelopmentObj;
}
export interface SyscapDeviceObj {
    general: string[];
    custom: object[];
}
export interface ProductionObj {
    addedSysCaps: string[];
    removedSysCaps: string[];
}
export interface DevelopmentObj {
    addedSysCaps: string[];
}
