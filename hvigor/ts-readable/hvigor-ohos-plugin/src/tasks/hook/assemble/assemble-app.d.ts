/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/assemble/assemble-app.d.ts
 */
import { DefaultTask, Project } from '@ohos/hvigor';
import { ProjectTaskService } from '../../service/project-task-service';
/**
 * 组装打包app的任务流
 *
 * @since 2022/1/20
 */
export declare class AssembleApp extends DefaultTask {
    private readonly _taskService;
    constructor(project: Project, taskService: ProjectTaskService);
    private initTaskDepends;
    registryAction: () => () => void;
}
