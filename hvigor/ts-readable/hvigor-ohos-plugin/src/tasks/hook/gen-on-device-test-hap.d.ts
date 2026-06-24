/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/gen-on-device-test-hap.d.ts
 */
import { ModuleTaskService } from '../service/module-task-service';
import { TargetTaskService } from '../service/target-task-service';
import { AbstractModuleHookTask } from './abstract-module-hook-task';
/**
 * 在Hsp/har中组装Hap打包的任务流,用于生成ohosTest的Hap包
 *
 * @since 2024/3/5
 */
export declare class GenOnDeviceTestHap extends AbstractModuleHookTask {
    constructor(moduleService: ModuleTaskService, isFaMode: boolean);
    initTaskDepends(taskTargetService: TargetTaskService): void;
    /**
     * 是否跳过生成带签名的hap的步骤
     *
     * @private
     */
    private skipSignHap;
}
