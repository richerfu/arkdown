/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/before-generate-device-coverage.d.ts
 */
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
/**
 * 命令行生成覆盖率测试报告的前置任务，用于依赖挂载
 */
export declare class BeforeGenerateDeviceCoverage extends OhosHapTask {
    constructor(taskService: TargetTaskService);
    protected doTaskAction(): void;
    initTaskDepends(): void;
}
