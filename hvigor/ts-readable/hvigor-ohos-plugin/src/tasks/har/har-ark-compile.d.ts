/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/har-ark-compile.d.ts
 */
import { TaskDetails, TaskInputValue, FileSet } from '@ohos/hvigor';
import { ProjectConfig } from '@ohos/hvigor-arkts-compose';
import { CodeType } from '../../enum/code-type-enum';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { ArkCompile } from '../ark-compile';
import { TargetTaskService } from '../service/target-task-service';
export declare class HarArkCompile extends ArkCompile {
    protected _log: OhosLogger;
    constructor(taskService: TargetTaskService, codeType: CodeType, taskDetails: TaskDetails);
    protected allowToPreloadSystemSo(): boolean;
    protected addPreloadSystemSoFileList(): void;
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    protected initDefaultArkCompileConfig(): Promise<ProjectConfig>;
    initTaskDepends(): void;
    private getDependencyKeysByPkgPath;
    private getDependencyKeys;
}
