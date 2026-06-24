/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-sign-lite-bin.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { BaseSign } from '../base/base-sign';
import { TargetTaskService } from '../service/target-task-service';
import { ModuleTargetData } from '../data/hap-task-target-data';
/**
 * 签名bin包
 */
export declare class LegacySignLiteBin extends BaseSign {
    protected readonly inputFile: string | undefined;
    protected readonly outputFile: string | undefined;
    protected readonly targetServiceData: ModuleTargetData | undefined;
    constructor(taskService: TargetTaskService);
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    taskShouldDo(): boolean;
    initTaskDepends(): void;
    protected doTaskAction(): Promise<void>;
}
