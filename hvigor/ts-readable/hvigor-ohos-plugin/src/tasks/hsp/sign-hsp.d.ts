/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/sign-hsp.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { BaseSign } from '../base/base-sign';
import { TargetTaskService } from '../service/target-task-service';
/**
 * Sign hsp
 *
 * @since 2023/1/17
 */
export declare class SignHsp extends BaseSign {
    protected readonly inputFile: string | undefined;
    protected readonly outputFile: string | undefined;
    constructor(targetService: TargetTaskService);
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
