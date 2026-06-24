/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/base/base-pack-app-task.d.ts
 */
import { Project, TaskDetails } from '@ohos/hvigor';
import { ProjectPathInfoIml } from '../../common/iml/project-path-info-iml';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { ProjectTaskService } from '../service/project-task-service';
import { OhosAppTask } from '../task/ohos-app-task';
/**
 * APP打包任务基类，（公共方法、公共属性）
 *
 * @since 2023/1/13
 */
export declare abstract class BasePackAppTask extends OhosAppTask {
    readonly _log: OhosLogger;
    readonly packageTool: string;
    readonly packInfoPath: string;
    readonly packResPath: string;
    readonly entryCardDir: string;
    readonly needPackRes: boolean;
    readonly pathInfo: ProjectPathInfoIml;
    protected constructor(project: Project, taskService: ProjectTaskService, taskDetails: TaskDetails);
    declareExecutionTool(): string;
    protected abstract initTaskDepends(): void;
    protected abstract doTaskAction(): void;
}
