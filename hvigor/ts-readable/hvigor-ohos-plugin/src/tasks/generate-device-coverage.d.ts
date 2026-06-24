/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-device-coverage.d.ts
 */
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
/**
 * 命令行生成ohos测试覆盖率报告
 */
export declare class GenerateDeviceCoverage extends OhosHapTask {
    private readonly _log;
    constructor(taskService: TargetTaskService);
    protected doTaskAction(): Promise<void>;
    getOutputPackagePath(): string;
    initTaskDepends(): void;
    protected getDependenciesPaths(): string[];
}
