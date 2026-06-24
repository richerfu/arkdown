/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/preview-compile-resource.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { AbstractPreviewerCompileResource } from './abstract/abstract-previewer-compile-resource';
import { ModuleTargetData } from './data/hap-task-target-data';
import { TargetTaskService } from './service/target-task-service';
/**
 * prview场景增量方式编译资源
 *
 * @since 2022/09/22
 */
export declare class PreviewCompileResource extends AbstractPreviewerCompileResource {
    private readonly _log;
    protected processProfileTask: string;
    declareExecutionCommand(): string;
    constructor(targetTaskService: TargetTaskService);
    initTaskDepends(): void;
    declareInputFiles(): FileSet;
    doTaskAction(): Promise<void>;
    invokeRestool(targetData: ModuleTargetData): Promise<void>;
}
