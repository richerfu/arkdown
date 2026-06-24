/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-process-profile.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
import { TaskInputValue } from '@ohos/hvigor';
import { FileSet } from '@ohos/hvigor';
/**
 * 处理config.json
 *
 * @since 2022/1/10
 */
export declare class LegacyProcessProfile extends OhosHapTask {
    private _log;
    private readonly _arkEnable;
    private readonly _deviceTypes;
    private readonly _intermediatesMergeLegacyProfile;
    private readonly _processedModuleJson;
    private readonly _moduleBuildProfilePath;
    constructor(taskService: TargetTaskService);
    initTaskDepends(): void;
    doTaskAction(): void;
    private replaceTargetAbilityPages;
    private replaceAbilityPage;
    private validateTargetAbilityName;
    private printInvalidError;
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
}
