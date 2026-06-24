/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/process-project-privacy-profile.d.ts
 */
import { FileSet, Project, TaskInputValue } from '@ohos/hvigor';
import { ProjectTaskService } from '../service/project-task-service';
import { OhosAppTask } from '../task/ohos-app-task';
export declare class ProcessProjectPrivacyProfile extends OhosAppTask {
    private outputPath;
    private appPacJson;
    constructor(project: Project, taskService: ProjectTaskService);
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected initTaskDepends(): void;
    protected doTaskAction(): void;
    private processValidPacFiles;
}
