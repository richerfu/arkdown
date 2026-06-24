/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-preview-compile-resource.d.ts
 */
import { AbstractPreviewerCompileResource } from '../abstract/abstract-previewer-compile-resource';
import { ModuleTargetData } from '../data/hap-task-target-data';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 老模型的Preview适配Restool资源增量编译任务
 *
 * @since 2022/1/20
 */
export declare class LegacyPreviewCompileResource extends AbstractPreviewerCompileResource {
    protected processProfileTask: string;
    private _log;
    declareExecutionCommand(): string;
    constructor(taskService: TargetTaskService, entryModuleName?: string);
    initTaskDepends(): void;
    protected initHarModuleDepends(): void;
    doTaskAction(): Promise<void>;
    invokeRestool(targetData: ModuleTargetData): Promise<void>;
}
