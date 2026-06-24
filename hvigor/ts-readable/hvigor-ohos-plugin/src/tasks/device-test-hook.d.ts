/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/device-test-hook.d.ts
 */
import { AbstractModuleHookTask } from './hook/abstract-module-hook-task';
import { ModuleTaskService } from './service/module-task-service';
import { TargetTaskService } from './service/target-task-service';
/**
 * 不依赖DevEco Studio的ohos测试构建任务
 *
 * @since 2023/12/19
 */
export declare class DeviceTestHook extends AbstractModuleHookTask {
    private _log;
    initTaskDepends(taskTargetService: TargetTaskService): void;
    constructor(moduleTaskService: ModuleTaskService, isFaMode: boolean);
    registryAction: () => Function;
}
