/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har-deduplicate/collect-duplicate-har-entries-ark-compile.d.ts
 */
import { FileSet, TaskDetails } from '@ohos/hvigor';
import { ProjectConfig } from '@ohos/hvigor-arkts-compose';
import { CodeType } from '../../enum/code-type-enum';
import { ArkCompile } from '../ark-compile';
import { TargetTaskService } from '../service/target-task-service';
/**
 * har包去重任务，har包去重时需要先编译hsp搜集到需要去重har的文件列表打到hap中
 */
export declare class CollectDuplicateHarEntriesArkCompile extends ArkCompile {
    private readonly hspNeedDeduplicateHarInfoMap;
    constructor(taskService: TargetTaskService, codeType: CodeType, taskDetails: TaskDetails);
    protected initDefaultArkCompileConfig(): Promise<ProjectConfig>;
    taskShouldDo(): boolean;
    declareOutputFiles(): FileSet;
}
