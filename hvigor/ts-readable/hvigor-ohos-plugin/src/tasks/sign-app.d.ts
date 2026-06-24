/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-app.d.ts
 */
import { Project, FileSet, TaskInputValue } from '@ohos/hvigor';
import { ProjectTaskService } from './service/project-task-service';
import { OhosAppTask } from './task/ohos-app-task';
/**
 * 对app包进行签名
 *
 * @since 2022/1/22
 */
export declare class SignApp extends OhosAppTask {
    private readonly log;
    private pathInfo;
    private signUtil;
    private signingConfig;
    constructor(project: Project, taskService: ProjectTaskService);
    get appWithSignedPkg(): boolean;
    beforeAlwaysAction(): Promise<void>;
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
