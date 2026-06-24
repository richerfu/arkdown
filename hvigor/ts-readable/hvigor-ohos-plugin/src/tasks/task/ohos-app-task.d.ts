/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/ohos-app-task.d.ts
 */
import { Project, TaskDetails } from '@ohos/hvigor';
import { IncrementalExecTask } from '@ohos/hvigor';
import { ProjectTaskService } from '../service/project-task-service';
/**
 * openHarmony基础app task
 *
 * @since 2022/1/10
 */
export declare abstract class OhosAppTask extends IncrementalExecTask {
    protected project: Project;
    protected service: ProjectTaskService;
    protected constructor(project: Project, taskService: ProjectTaskService, taskDetails: TaskDetails);
    protected abstract initTaskDepends(): void;
    /**
     * 为每个target执行task的具体逻辑
     *
     */
    protected abstract doTaskAction(): void;
    registryAction: () => Function;
}
