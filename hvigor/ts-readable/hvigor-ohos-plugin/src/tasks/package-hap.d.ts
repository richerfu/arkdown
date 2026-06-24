/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/package-hap.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { BasePackHapTask } from './base/base-pack-hap-task';
import { TargetTaskService } from './service/target-task-service';
/**
 * 打包Hap包
 *
 * @since 2022/1/10
 */
export declare class PackageHap extends BasePackHapTask {
    private readonly appOptObj;
    private readonly targetModuleOptObj;
    private readonly targetJsonPath;
    constructor(taskService: TargetTaskService);
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    private setPriorResourcePath;
    beforeAlwaysAction(): Promise<void>;
    protected doTaskAction(): Promise<void>;
}
