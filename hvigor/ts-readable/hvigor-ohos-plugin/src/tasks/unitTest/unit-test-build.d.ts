/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/unit-test-build.d.ts
 */
import { AbstractModuleHookTask } from '../hook/abstract-module-hook-task';
import { ModuleTaskService } from '../service/module-task-service';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 单元测试构建任务
 *
 * @since 2023/4/23
 */
export declare class UnitTestBuild extends AbstractModuleHookTask {
    constructor(moduleTaskService: ModuleTaskService, isFaMode: boolean);
    initTaskDepends(taskTargetService: TargetTaskService): void;
}
