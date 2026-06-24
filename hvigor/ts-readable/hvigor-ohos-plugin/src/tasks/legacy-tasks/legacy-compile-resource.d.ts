/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-compile-resource.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { AbstractCompileResource } from '../abstract/abstract-compile-resource';
import { ModuleTargetData } from '../data/hap-task-target-data';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 老模型的资源编译任务
 *
 * @since 2022/1/20
 */
export declare class LegacyCompileResource extends AbstractCompileResource {
    protected processProfileTask: string;
    private _log;
    constructor(taskService: TargetTaskService, entryModuleName?: string);
    declareExecutionCommand(): string;
    declareInputFiles(): FileSet;
    initTaskDepends(): void;
    protected initHarModuleDepends(): void;
    doTaskAction(): Promise<void>;
    invokeRestool(targetData: ModuleTargetData): Promise<void>;
}
