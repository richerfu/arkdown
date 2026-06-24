/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/fast-package-hsp.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { BasePackHapTask } from '../base/base-pack-hap-task';
import { TargetTaskService } from '../service/target-task-service';
/**
 * Fast packaging hsp task
 *
 * @since 2025/2/27
 */
export declare class FastPackageHsp extends BasePackHapTask {
    constructor(taskService: TargetTaskService);
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    beforeAlwaysAction(): Promise<void>;
    protected doTaskAction(): Promise<void>;
}
