/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/process-module-privacy-profile.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosModuleTask } from '../task/ohos-module-task';
import { FileSet } from '@ohos/hvigor';
export declare class ProcessModulePrivacyProfile extends OhosModuleTask {
    private _log;
    private readonly originalModulePacJsonPath;
    private readonly processedPacJsonPath;
    constructor(taskService: TargetTaskService);
    taskShouldDo(): boolean;
    protected doTaskAction(): void;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    initTaskDepends(): void;
}
