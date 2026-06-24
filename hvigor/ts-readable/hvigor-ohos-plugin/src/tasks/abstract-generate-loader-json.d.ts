/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract-generate-loader-json.d.ts
 */
import { TaskDetails, TaskInputValue } from '@ohos/hvigor';
import { OhosLogger } from '../utils/log/ohos-logger';
import { TargetTaskService } from './service/target-task-service';
import { OhosHapTask } from './task/ohos-hap-task';
/**
 * 生成loader需要的json文件
 *
 * @since 2022/9/9
 */
export declare abstract class AbstractGenerateLoaderJson extends OhosHapTask {
    protected _log: OhosLogger;
    protected workerConfig: string[] | undefined;
    protected modulePathMap: Record<string, string>;
    protected readonly projectRootPath: string;
    protected constructor(taskService: TargetTaskService, taskDetails: TaskDetails);
    declareInputs(): Map<string, TaskInputValue>;
    protected beforeTask(): void;
    protected getWorkerConfig(): string[] | undefined;
    private getModulePathMap;
}
