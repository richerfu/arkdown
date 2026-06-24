/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/package-sign-har.d.ts
 */
import { FileSet, TaskInputValue } from '@ohos/hvigor';
import { BaseSign } from '../base/base-sign';
import { TargetTaskService } from '../service/target-task-service';
/**
 * Sign&Package har
 *
 * @since 2024/05/11
 */
export declare class PackageSignHar extends BaseSign {
    protected readonly compiledRoot: string;
    protected readonly taskTmpRoot: string;
    protected readonly outputFile: string;
    protected readonly packageRoot: string;
    constructor(targetService: TargetTaskService);
    protected beforeTask(): void;
    declareInputs(): Map<string, TaskInputValue>;
    taskShouldDo(): boolean;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
