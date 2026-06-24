/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/ohosTest/generate-ohos-test-template.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { TargetTaskService } from '../service/target-task-service';
import { OhosModuleTask } from '../task/ohos-module-task';
/**
 * 不依赖DevEco Studio的单元测试构建任务的前置任务
 *
 * @since 2024/05/16
 */
export declare class GenerateOhosTestTemplate extends OhosModuleTask {
    constructor(taskService: TargetTaskService);
    private getParams;
    declareOutputFiles(): FileSet;
    declareInputs(): Map<string, TaskInputValue>;
    doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
