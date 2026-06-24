/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pre-check-syscap.d.ts
 */
import { TaskDetails } from '@ohos/hvigor';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
/**
 * syscap相关配置预检查任务
 */
export declare class PreCheckSyscap extends OhosHapTask {
    private readonly _log;
    private readonly sysCapJsonPath;
    private readonly sourceRoot;
    private readonly jsonFilePath;
    constructor(targetTaskService: TargetTaskService, taskDetails: TaskDetails);
    protected doTaskAction(): void;
    initTaskDepends(): void;
}
