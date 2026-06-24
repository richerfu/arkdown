/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har-deduplicate/process-id-defined-file.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * har包去重场景，用户可以配置一个全局的idDefinedFilePath指向一个全局资源ID固定表。
 * 在IDE中在这个文件是一个json5配置文件，但restool只能识别json文件
 * 所以要先把json5文件转成成json文件之后传递给restool的resConfig.json中的definedIds字段。
 */
export declare class ProcessIdDefinedFile extends OhosHapTask {
    private readonly customIdDefinedFilePath;
    private readonly idDefinedJsonFile;
    declareInputFiles(): FileSet;
    constructor(taskService: TargetTaskService);
    taskShouldDo(): boolean;
    protected beforeTask(): void;
    protected doTaskAction(): void;
    initTaskDepends(): void;
    private getCustomIdDefinedFilePath;
}
